//! `#[facet_hdl::rpc]`: turn `#[export]` fns in a module into a boundary the
//! fabric calls them through.
//!
//! Only the export attributes and fn signatures are parsed. Everything else
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
    keyword KMut = "mut";
    keyword KExport = "export";
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

    enum ExportOpt {
        Oneway(KOneway),
        Depth(Cons<KDepth, Assign, LiteralInteger>),
    }

    struct ExportAttr {
        pound: Pound,
        body: BracketGroupContaining<Cons<KExport, Option<ParenthesisGroupContaining<CommaDelimitedVec<ExportOpt>>>>>,
    }

    /// Everything of an exported fn up to its parameter list; the return
    /// type and body are split off by hand (types contain `<..,..>`, which
    /// aren't token groups).
    struct FnHead {
        export: ExportAttr,
        attrs: Vec<Attr>,
        vis: Option<Vis>,
        kw: KFn,
        name: Ident,
        params: ParenthesisGroup,
    }
}

const DEFAULT_DEPTH: usize = 4;

struct Export {
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
    let mut exports = Vec::new();
    let mut it = module.body.0.stream().to_token_iter();
    loop {
        if let Ok(head) = FnHead::parse(&mut it) {
            let (ret, body) = split_ret_and_body(&mut it)?;
            let export = export_of(&head, ret.clone())?;
            // The fn stays exactly as written, minus `#[export]`, so Rust can
            // still call it directly.
            let (attrs, vis, kw, name, params) = (&head.attrs, &head.vis, &head.kw, &head.name, &head.params);
            let arrow = ret.as_ref().map(|r| quote! { -> #r });
            kept.extend(quote! { #attrs #vis #kw #name #params #{arrow} #body });
            exports.push(export);
        } else if let Ok(tt) = TokenTree::parse(&mut it) {
            kept.extend(Some(tt));
        } else {
            break;
        }
    }
    if exports.is_empty() {
        return Err("#[facet_hdl::rpc] module has no #[export] fns".into());
    }

    let boundary = Ident::new(&camel(&module.name.to_string()), module.name.span());
    let generated = generate(&boundary, &exports);
    let (attrs, vis, kw, name) = (&module.attrs, &module.vis, &module.kw, &module.name);
    Ok(quote! { #attrs #vis #kw #name { #kept #generated } })
}

/// After the parameter list: `-> Type` (optional) and then the body.
fn split_ret_and_body(it: &mut TokenIter) -> std::result::Result<(Option<TokenStream>, Group), String> {
    let mut ret = TokenStream::new();
    let mut seen_arrow = false;
    loop {
        let Ok(tt) = TokenTree::parse(it) else {
            return Err("exported fn has no body".into());
        };
        match &tt {
            TokenTree::Group(g) if g.delimiter() == Delimiter::Brace => {
                return Ok((seen_arrow.then_some(ret), g.clone()));
            }
            TokenTree::Ident(i) if i == "where" => {
                return Err("exported fns can't be generic".into());
            }
            TokenTree::Punct(p) if !seen_arrow && p.as_char() == '-' => {}
            TokenTree::Punct(p) if !seen_arrow && p.as_char() == '>' => seen_arrow = true,
            _ if seen_arrow => ret.extend(Some(tt)),
            _ => return Err(format!("unexpected `{tt}` after an exported fn's parameters")),
        }
    }
}

fn export_of(head: &FnHead, ret: Option<TokenStream>) -> std::result::Result<Export, String> {
    let name = head.name.clone();
    let (mut oneway, mut depth) = (false, DEFAULT_DEPTH);
    if let Some(opts) = &head.export.body.content.second {
        for opt in opts.content.iter() {
            match &opt.value {
                ExportOpt::Oneway(_) => oneway = true,
                ExportOpt::Depth(d) => {
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
    if params.is_empty() {
        // An empty args struct has no bits and can't be a queue entry.
        return Err(format!(
            "`{name}` takes no arguments; fabric calls need at least one for now"
        ));
    }
    Ok(Export {
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

fn generate(boundary: &Ident, exports: &[Export]) -> TokenStream {
    let mut items = TokenStream::new();
    let mut ports = TokenStream::new();
    let mut arms = TokenStream::new();
    for e in exports {
        let args = Ident::new(&format!("{}Args", camel(&e.name.to_string())), e.name.span());
        let req = Ident::new(&format!("{}_req", e.name), e.name.span());
        let resp = Ident::new(&format!("{}_resp", e.name), e.name.span());
        let depth = Literal::usize_unsuffixed(e.depth);
        let fname = &e.name;

        let mut fields = TokenStream::new();
        let mut call_args = TokenStream::new();
        for (p, ty) in &e.params {
            fields.extend(quote! { pub #p: #ty, });
            call_args.extend(quote! { args.#p, });
        }
        let doc = Literal::string(&format!(" Arguments of a fabric call to [`{fname}`]."));
        items.extend(quote! {
            #[doc = #doc]
            #[derive(::facet::Facet)]
            pub struct #args { #fields }
        });

        ports.extend(quote! { pub #req: ::facet_hdl::FromFabricQueue<#args, #depth>, });
        let invoke = quote! { ::facet_hdl::rpc::call(move || self::#fname(#call_args)) };
        if e.oneway {
            // Nobody is listening for the result; a panic is only reported by
            // the panic hook.
            arms.extend(quote! {
                if let ::core::option::Option::Some(args) = self.#req.pop()? {
                    let _ = #invoke;
                    handled += 1;
                }
            });
        } else {
            let ret = e.ret.clone().unwrap_or_else(|| quote! { () });
            ports.extend(quote! {
                pub #resp: ::facet_hdl::ToFabricQueue<::facet_hdl::rpc::CallResult<#ret>, #depth>,
            });
            arms.extend(quote! {
                if let ::core::option::Option::Some(args) = self.#req.pop()? {
                    let result = #invoke;
                    ::facet_hdl::rpc::respond(&mut self.#resp, &result)?;
                    handled += 1;
                }
            });
        }
    }
    let first_req = Ident::new(&format!("{}_req", exports[0].name), exports[0].name.span());
    quote! {
        #items

        /// The boundary the fabric calls through: generate its VHDL from this
        /// type, `bind` it, then run [`Self::serve`].
        #[derive(::facet::Facet)]
        pub struct #boundary { #ports }

        impl #boundary {
            /// Answers at most one waiting call per fn. Returns how many it answered.
            pub fn poll(&mut self) -> ::core::result::Result<usize, ::facet_hdl::PortError> {
                let mut handled = 0usize;
                #arms
                ::core::result::Result::Ok(handled)
            }

            /// Answers calls forever, sleeping on the interrupt while there are none.
            pub fn serve(&mut self) -> ::core::result::Result<::core::convert::Infallible, ::facet_hdl::PortError> {
                loop {
                    if self.poll()? == 0 {
                        ::facet_hdl::rpc::wait(&self.#first_req, ::core::time::Duration::from_secs(1))?;
                    }
                }
            }
        }
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
        assert!(!out.contains("export"), "the attribute is consumed: {out}");
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
    fn refuses_what_it_cant_carry() {
        assert!(err("mod m { #[export(oneway)] fn f(x: u8) -> u8 { x } }").contains("oneway"));
        assert!(err("mod m { #[export] fn f() -> u8 { 1 } }").contains("at least one"));
        assert!(err("mod m { fn f(x: u8) {} }").contains("no #[export]"));
        assert!(err("mod m { #[export] fn f(&self, x: u8) {} }").contains("free functions"));
        assert!(err("mod m { #[export] fn f(x: u8) where u8: Copy {} }").contains("generic"));
        assert!(err("mod m { #[export(depth = 3)] fn f(x: u8) {} }").contains("power of two"));
        assert!(err("fn f() {}").contains("inline `mod"));
    }
}
