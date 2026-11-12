//! `#[facet_hdl::rpc]`: calls across the HPS<->fabric boundary, declared as
//! Rust fns in a module. `#[export]` fns are Rust the fabric calls;
//! `#[import]` fns are bodiless declarations of HDL that Rust calls.
//!
//! Only those attributes and the fn signatures are parsed. Everything else
//! in the module is passed through untouched, which is why this needs
//! unsynn's few building blocks and not a whole Rust grammar.

// unsynn's `Error` is large, and its grammar macro returns it from every
// parser it generates. unsynn allows this lint itself; see the PLANNED note at
// the top of unsynn 0.3's lib.rs. Remove once that Error is boxed or shrunk.
#![allow(clippy::result_large_err)]

use unsynn::*;

unsynn! {
    keyword KFn = "fn";
    keyword KMod = "mod";
    keyword KPub = "pub";
    keyword KExport = "export";
    keyword KImport = "import";
    keyword KOneway = "oneway";
    keyword KDepth = "depth";

    struct Vis {
        kw: KPub,
        scope: Option<ParenthesisGroup>,
    }

    struct Attr {
        pound: Pound,
        body: BracketGroup,
    }

    struct Module {
        attrs: Vec<Attr>,
        vis: Option<Vis>,
        kw: KMod,
        name: Ident,
        body: BraceGroup,
    }

    enum Way {
        Export(KExport),
        Import(KImport),
    }

    enum CallOpt {
        Oneway(KOneway),
        Depth(Cons<KDepth, Assign, LiteralInteger>),
    }

    struct CallAttr {
        pound: Pound,
        body: BracketGroupContaining<Cons<Way, Option<ParenthesisGroupContaining<CommaDelimitedVec<CallOpt>>>>>,
    }

    /// A marked fn up to its parameter list; the return type and body are
    /// split off by hand (types contain `<..,..>`, which aren't token groups).
    struct FnHead {
        call: CallAttr,
        attrs: Vec<Attr>,
        vis: Option<Vis>,
        kw: KFn,
        name: Ident,
        params: ParenthesisGroup,
    }
}

const DEFAULT_DEPTH: usize = 4;

#[derive(Clone, Copy, PartialEq)]
enum Dir {
    /// Rust fn, fabric calls it.
    Export,
    /// HDL, Rust calls it.
    Import,
}

struct Call {
    dir: Dir,
    name: Ident,
    params: Vec<(Ident, TokenStream)>,
    ret: Option<TokenStream>,
    oneway: bool,
    depth: usize,
}

#[proc_macro_attribute]
pub fn rpc(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    match expand(attr.into(), item.into()) {
        Ok(ts) => ts.into(),
        Err(msg) => {
            let msg = Literal::string(&msg);
            quote! { ::core::compile_error!(#msg); }.into()
        }
    }
}

fn expand(attr: TokenStream, item: TokenStream) -> std::result::Result<TokenStream, String> {
    if !attr.is_empty() {
        return Err("#[facet_hdl::rpc] takes no arguments".into());
    }
    let module = Module::parse_all(&mut item.to_token_iter())
        .map_err(|e| format!("#[facet_hdl::rpc] goes on an inline `mod name {{ .. }}`: {e}"))?;

    let mut kept = TokenStream::new();
    let mut calls = Vec::new();
    let mut it = module.body.0.stream().to_token_iter();
    loop {
        if let Ok(head) = FnHead::parse(&mut it) {
            let dir = match head.call.body.content.first {
                Way::Export(_) => Dir::Export,
                Way::Import(_) => Dir::Import,
            };
            let (ret, body) = split_ret_and_body(&mut it, &head.name, dir)?;
            let call = call_of(&head, dir, ret.clone())?;
            if let Some(body) = body {
                // An export stays exactly as written, minus `#[export]`, so
                // Rust can still call it directly. An import has no body; it
                // becomes a method on the boundary instead.
                let (attrs, vis, kw, name, params) = (&head.attrs, &head.vis, &head.kw, &head.name, &head.params);
                let arrow = ret.as_ref().map(|r| quote! { -> #r });
                kept.extend(quote! { #attrs #vis #kw #name #params #{arrow} #body });
            }
            calls.push(call);
        } else if let Ok(tt) = TokenTree::parse(&mut it) {
            kept.extend(Some(tt));
        } else {
            break;
        }
    }
    if calls.is_empty() {
        return Err("#[facet_hdl::rpc] module has no #[export] or #[import] fns".into());
    }

    let boundary = Ident::new(&camel(&module.name.to_string()), module.name.span());
    let generated = generate(&boundary, &calls);
    let (attrs, vis, kw, name) = (&module.attrs, &module.vis, &module.kw, &module.name);
    Ok(quote! { #attrs #vis #kw #name { #kept #generated } })
}

/// After the parameter list: `-> Type` (optional), then a body for an export
/// or `;` for an import.
fn split_ret_and_body(
    it: &mut TokenIter,
    name: &Ident,
    dir: Dir,
) -> std::result::Result<(Option<TokenStream>, Option<Group>), String> {
    let mut ret = TokenStream::new();
    let mut seen_arrow = false;
    loop {
        let Ok(tt) = TokenTree::parse(it) else {
            return Err(match dir {
                Dir::Export => format!("exported fn `{name}` has no body"),
                Dir::Import => format!("imported fn `{name}` must end in `;`"),
            });
        };
        match &tt {
            TokenTree::Group(g) if g.delimiter() == Delimiter::Brace => {
                if dir == Dir::Import {
                    return Err(format!(
                        "imported fn `{name}` is implemented in HDL, so it has no body: end it with `;`"
                    ));
                }
                return Ok((seen_arrow.then_some(ret), Some(g.clone())));
            }
            TokenTree::Punct(p) if p.as_char() == ';' => {
                if dir == Dir::Export {
                    return Err(format!("exported fn `{name}` needs a body"));
                }
                return Ok((seen_arrow.then_some(ret), None));
            }
            TokenTree::Ident(i) if i == "where" => {
                return Err(format!("`{name}` can't be generic"));
            }
            TokenTree::Punct(p) if !seen_arrow && p.as_char() == '-' => {}
            TokenTree::Punct(p) if !seen_arrow && p.as_char() == '>' => seen_arrow = true,
            _ if seen_arrow => ret.extend(Some(tt)),
            _ => return Err(format!("unexpected `{tt}` after `{name}`'s parameters")),
        }
    }
}

fn call_of(head: &FnHead, dir: Dir, ret: Option<TokenStream>) -> std::result::Result<Call, String> {
    let name = head.name.clone();
    let (mut oneway, mut depth) = (false, DEFAULT_DEPTH);
    if let Some(opts) = &head.call.body.content.second {
        for opt in opts.content.iter() {
            match &opt.value {
                CallOpt::Oneway(_) => oneway = true,
                CallOpt::Depth(d) => {
                    depth = d.third.value() as usize;
                    if !depth.is_power_of_two() || !(2..=65536).contains(&depth) {
                        return Err(format!("`{name}`: depth must be a power of two from 2 to 65536"));
                    }
                }
            }
        }
    }
    let ret = ret.filter(|r| r.to_string().replace(' ', "") != "()");
    if oneway && ret.is_some() {
        return Err(format!(
            "`{name}` is oneway, so nothing can carry its return value back; return () or drop `oneway`"
        ));
    }
    let params = params_of(&name, head.params.0.stream())?;
    Ok(Call {
        dir,
        name,
        params,
        ret,
        oneway,
        depth,
    })
}

/// `a: u8, mut b: Result<u16, E>` -> names and type tokens. Commas inside
/// `<..>` belong to the type.
fn params_of(fn_name: &Ident, ts: TokenStream) -> std::result::Result<Vec<(Ident, TokenStream)>, String> {
    let mut pieces = vec![Vec::new()];
    let mut angle = 0i32;
    for tt in ts {
        if let TokenTree::Punct(p) = &tt {
            match p.as_char() {
                '<' => angle += 1,
                '>' => angle -= 1,
                ',' if angle == 0 => {
                    pieces.push(Vec::new());
                    continue;
                }
                _ => {}
            }
        }
        pieces.last_mut().unwrap().push(tt);
    }
    pieces
        .into_iter()
        .filter(|p| !p.is_empty())
        .map(|p| {
            // `self`, `&self`, `&mut self`, `mut self: Box<Self>`...
            let before_colon = p
                .iter()
                .take_while(|t| !matches!(t, TokenTree::Punct(c) if c.as_char() == ':'));
            if before_colon
                .into_iter()
                .any(|t| matches!(t, TokenTree::Ident(i) if i == "self"))
            {
                return Err(format!("`{fn_name}` takes self; exported fns must be free functions"));
            }
            let mut toks = p.into_iter().peekable();
            if matches!(toks.peek(), Some(TokenTree::Ident(i)) if i == "mut") {
                toks.next();
            }
            let bad = || format!("`{fn_name}`: parameters must be `name: Type`");
            let Some(TokenTree::Ident(name)) = toks.next() else {
                return Err(bad());
            };
            match toks.next() {
                Some(TokenTree::Punct(p)) if p.as_char() == ':' => {}
                _ => return Err(bad()),
            }
            Ok((name, toks.collect()))
        })
        .collect()
}

fn generate(boundary: &Ident, calls: &[Call]) -> TokenStream {
    let mut items = TokenStream::new();
    let mut ports = TokenStream::new();
    let mut arms = TokenStream::new();
    let mut methods = TokenStream::new();
    for c in calls {
        let fname = &c.name;
        let req = Ident::new(&format!("{fname}_req"), fname.span());
        let resp = Ident::new(&format!("{fname}_resp"), fname.span());
        let depth = Literal::usize_unsuffixed(c.depth);
        let ret = c.ret.clone().unwrap_or_else(|| quote! { () });

        // No parameters: the call is the handshake itself, a `()` entry.
        let (args_ty, args_value, call_args) = if c.params.is_empty() {
            (quote! { () }, quote! { () }, TokenStream::new())
        } else {
            let args = Ident::new(&format!("{}Args", camel(&fname.to_string())), fname.span());
            let (mut fields, mut call_args, mut inits) = (TokenStream::new(), TokenStream::new(), TokenStream::new());
            for (p, ty) in &c.params {
                fields.extend(quote! { pub #p: #ty, });
                call_args.extend(quote! { args.#p, });
                inits.extend(quote! { #p, });
            }
            let doc = Literal::string(&format!(" Arguments of a call to `{fname}`."));
            items.extend(quote! {
                #[doc = #doc]
                #[derive(::facet::Facet)]
                pub struct #args { #fields }
            });
            (quote! { #args }, quote! { #args { #inits } }, call_args)
        };

        match c.dir {
            Dir::Export => {
                ports.extend(quote! { pub #req: ::facet_hdl::FromFabricQueue<#args_ty, #depth>, });
                let bind_args = (!c.params.is_empty()).then(|| quote! { args });
                let pattern = bind_args.unwrap_or_else(|| quote! { () });
                let invoke = quote! { ::facet_hdl::rpc::call(move || self::#fname(#call_args)) };
                if c.oneway {
                    // Nobody is listening for the result; a panic is only
                    // reported by the panic hook.
                    arms.extend(quote! {
                        if let ::core::option::Option::Some(#pattern) = self.#req.pop()? {
                            let _ = #invoke;
                            handled += 1;
                        }
                    });
                } else {
                    ports.extend(quote! {
                        pub #resp: ::facet_hdl::ToFabricQueue<::facet_hdl::rpc::CallResult<#ret>, #depth>,
                    });
                    arms.extend(quote! {
                        if let ::core::option::Option::Some(#pattern) = self.#req.pop()? {
                            let result = #invoke;
                            ::facet_hdl::rpc::respond(&self.#resp, &result)?;
                            handled += 1;
                        }
                    });
                }
            }
            Dir::Import => {
                ports.extend(quote! { pub #req: ::facet_hdl::ToFabricQueue<#args_ty, #depth>, });
                let mut params = TokenStream::new();
                for (p, ty) in &c.params {
                    params.extend(quote! { #p: #ty, });
                }
                if c.oneway {
                    let doc = Literal::string(&format!(
                        " Hands the fabric's `{fname}` its arguments, waiting only for room in the queue."
                    ));
                    methods.extend(quote! {
                        #[doc = #doc]
                        pub async fn #fname(&self, #params) -> ::core::result::Result<(), ::facet_hdl::PortError> {
                            self.#req.send(&#args_value).await
                        }
                    });
                } else {
                    let doc = Literal::string(&format!(
                        " Calls the fabric's `{fname}`. Calls may overlap; dropping the future abandons the call."
                    ));
                    ports.extend(quote! { pub #resp: ::facet_hdl::rpc::Answers<#ret, #depth>, });
                    methods.extend(quote! {
                        #[doc = #doc]
                        pub async fn #fname(&self, #params) -> ::core::result::Result<#ret, ::facet_hdl::PortError> {
                            ::facet_hdl::rpc::invoke(&self.#req, &#args_value, &self.#resp).await
                        }
                    });
                }
            }
        }
    }

    // `poll`/`serve` answer the fabric, so only exports need them.
    if let Some(first) = calls.iter().find(|c| c.dir == Dir::Export) {
        let first_req = Ident::new(&format!("{}_req", first.name), first.name.span());
        methods.extend(quote! {
            /// Answers at most one waiting call per exported fn. Returns how many it answered.
            pub fn poll(&self) -> ::core::result::Result<usize, ::facet_hdl::PortError> {
                let mut handled = 0usize;
                #arms
                ::core::result::Result::Ok(handled)
            }

            /// Answers calls forever, sleeping on the interrupt while there are none.
            /// Runs alongside imported calls on the same boundary.
            pub async fn serve(&self) -> ::core::result::Result<::core::convert::Infallible, ::facet_hdl::PortError> {
                loop {
                    let idle = ::facet_hdl::rpc::Idle::start(&self.#first_req);
                    if self.poll()? == 0 {
                        idle.wait().await?;
                    }
                }
            }
        });
    }
    quote! {
        #items

        /// The boundary the calls go through: generate its Spade from this
        /// type, then `bind` it.
        #[derive(::facet::Facet)]
        pub struct #boundary { #ports }

        impl #boundary { #methods }
    }
}

/// `lookup_gain` -> `LookupGain`.
fn camel(snake: &str) -> String {
    snake
        .split('_')
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut c = s.chars();
            c.next()
                .map(|f| f.to_ascii_uppercase())
                .into_iter()
                .chain(c)
                .collect::<String>()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err(src: &str) -> String {
        let item: TokenStream = src.parse().unwrap();
        expand(TokenStream::new(), item).unwrap_err()
    }

    #[test]
    fn expands_and_keeps_the_fn_callable() {
        let item: TokenStream = "pub mod m { use x::Y; #[export(depth = 8)] pub fn add_two(a: u8, b: Result<u8, Y>) -> Option<u8> { None } }"
            .parse()
            .unwrap();
        let out = expand(TokenStream::new(), item).unwrap().to_string();
        assert!(out.contains("pub fn add_two"), "{out}");
        assert!(!out.contains("# [export"), "the attribute is consumed: {out}");
        assert!(out.contains("pub struct AddTwoArgs"), "{out}");
        assert!(
            out.contains("pub b : Result < u8 , Y >"),
            "a comma inside <..> stays in the type: {out}"
        );
        assert!(out.contains("FromFabricQueue < AddTwoArgs , 8 >"), "{out}");
        assert!(out.contains("CallResult < Option < u8 > >"), "{out}");
        assert!(out.contains("pub struct M"), "{out}");
    }

    #[test]
    fn unit_fns_still_answer_and_oneway_ones_dont() {
        let item: TokenStream = "mod m { #[export] fn a(x: u8) {} #[export(oneway)] fn b(x: u8) -> () {} }"
            .parse()
            .unwrap();
        let out = expand(TokenStream::new(), item).unwrap().to_string();
        assert!(out.contains("pub a_resp"), "{out}");
        assert!(out.contains("CallResult < () >"), "{out}");
        assert!(!out.contains("b_resp"), "{out}");
    }

    #[test]
    fn no_arguments_means_a_unit_entry() {
        let item: TokenStream = "mod m { #[export] fn tick() -> u8 { 1 } #[import] fn ping(); }"
            .parse()
            .unwrap();
        let out = expand(TokenStream::new(), item).unwrap().to_string();
        assert!(
            out.contains("tick_req : :: facet_hdl :: FromFabricQueue < () , 4 >"),
            "{out}"
        );
        assert!(
            out.contains("ping_req : :: facet_hdl :: ToFabricQueue < () , 4 >"),
            "{out}"
        );
        assert!(
            out.contains("ping_resp : :: facet_hdl :: rpc :: Answers < () , 4 >"),
            "{out}"
        );
        assert!(!out.contains("TickArgs"), "{out}");
    }

    #[test]
    fn imports_become_methods_and_lose_their_declaration() {
        let item: TokenStream =
            "mod m { #[import(depth = 8)] pub fn scale(x: u16) -> u32; #[import(oneway)] fn led(on: bool); }"
                .parse()
                .unwrap();
        let out = expand(TokenStream::new(), item).unwrap().to_string();
        assert!(!out.contains("pub fn scale (x"), "the bodiless fn is gone: {out}");
        assert!(out.contains("pub async fn scale (& self , x : u16 ,)"), "{out}");
        assert!(out.contains("Result < u32 , :: facet_hdl :: PortError >"), "{out}");
        assert!(
            out.contains("scale_req : :: facet_hdl :: ToFabricQueue < ScaleArgs , 8 >"),
            "{out}"
        );
        assert!(
            out.contains("scale_resp : :: facet_hdl :: rpc :: Answers < u32 , 8 >"),
            "{out}"
        );
        assert!(!out.contains("led_resp"), "{out}");
        assert!(!out.contains("fn serve"), "nothing to serve without exports: {out}");
    }

    #[test]
    fn refuses_what_it_cant_carry() {
        assert!(err("mod m { #[export(oneway)] fn f(x: u8) -> u8 { x } }").contains("oneway"));
        assert!(err("mod m { fn f(x: u8) {} }").contains("no #[export] or #[import]"));
        assert!(err("mod m { #[import] fn f(x: u8) -> u8 { x } }").contains("no body"));
        assert!(err("mod m { #[export] fn f(x: u8); }").contains("needs a body"));
        assert!(err("mod m { #[export] fn f(&self, x: u8) {} }").contains("free functions"));
        assert!(err("mod m { #[export] fn f(x: u8) where u8: Copy {} }").contains("generic"));
        assert!(err("mod m { #[export(depth = 3)] fn f(x: u8) {} }").contains("power of two"));
        assert!(err("fn f() {}").contains("inline `mod"));
    }
}
