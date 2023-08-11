#![feature(proc_macro_span)]
#![feature(proc_macro_quote)]
extern crate proc_macro2;

use std::fs;
use std::path;
use std::path::PathBuf;

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::spanned::Spanned;
use syn::{parse_file, parse_macro_input, Ident, Item, ItemFn, Stmt};

/// This procedural macro will generate a function that will build the IDT from
/// the module where all interrupts are defined.
/// This function will then have to be called from the main function.
///
/// ```
/// generate_idt();
/// ```
#[proc_macro_attribute]
pub fn interrupt_descriptor_table(
    args: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let offset = u32::from_str_radix(
        &args
            .to_string()
            .split("x")
            .collect::<Vec<&str>>()
            .get(1)
            .unwrap(),
        16,
    )
    .unwrap();
    let item_2 = TokenStream::from(item.clone());
    let item_1 = item;
    let module = parse_macro_input!(item_1 as syn::ItemMod);
    let mut mod_filename = format!(
        "{}/{}/handlers.rs",
        proc_macro::Span::call_site()
            .source_file()
            .path()
            .parent()
            .unwrap()
            .to_str()
            .unwrap(),
        module.ident.to_string()
    );
    let path = PathBuf::from(&mod_filename);
    let file = fs::read_to_string(&path).unwrap();
    let code = parse_file(&file).unwrap();
    let mut interrupts_token: Vec<TokenStream> = Vec::new();
    // Iterate over all 256 interrupts
    for i in 0..256 {
        let title = format!("_int0x{:x}", i);
        let title = title.as_str();
        let int_number = i as usize;
        let ident = Ident::new(
            &format!("{}{}", "int", int_number),
            syn::__private::Span::mixed_site(),
        );
        let fn_name = Ident::new(&title, Span::mixed_site());
        // Add statements to set handler's address for this entry in the IDT
        let code = quote! {
            let #ident = table.get_entry_mut(#int_number).unwrap();
            #ident.set_offset(interrupts::handlers::#fn_name as *const () as *const u8 as u32);
        };
        interrupts_token.push(code);
    }

    let default_table = quote! {
        // We create an empty table
        let mut table = Table::empty();
        let mut default : GateDescriptor = GateDescriptor::new();
        // Default type is Interrupt 32 bits
        default.set_type(GateType::InterruptGate32b);
        // Segment is hard coded but has to be passed as parameter in the future
        let mut segment : SegmentSelector = SegmentSelector::new()
            .with_gdt()
            .with_privilege(0b00)
            .with_segment_index(16);
        default.set_segment_selector(segment);
        default.set_valid();
        // We populate the table
        table.populate(default);
    };

    // We merge every streams.
    let stream = quote! {
        #item_2
        // Function name
        fn generate_idt() {
            #default_table
            #(#interrupts_token)*
            table.write(#offset);
        }
    };
    stream.into()
}

/// This proc macro aims to provide a higher level interface for interrupts definition.
///
/// This macro wraps the function in a naked wrapper function.
/// The wrapper function is defined as follows :
/// ```rust
/// use std::arch::asm;
///
/// #[naked]
/// pub fn _foo_naked_wrapper () {
///     unsafe { asm!(
///         "pushad
///         call _foo
///         popad
///         iretd", options(noreturn)
///     ) }
/// }
/// ```
#[proc_macro_attribute]
pub fn interrupt(
    args: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let arg = args.to_string();
    let arg = arg.as_str();
    let item_2 = TokenStream::from(item.clone());
    let func = parse_macro_input!(item as ItemFn);
    let ItemFn {
        attrs,
        vis,
        sig,
        block,
    } = func;

    match arg {
        // Handle specific arg (not implemented yet)
        _ => {
            let name = sig.ident.to_string();
            let wrapped_name = format!("_wrapped{}", name);

            /// Compute int number
            let int_number: u32 =
                u32::from_str_radix(name.split("x").collect::<Vec<&str>>().get(1).unwrap(), 16)
                    .unwrap();
            let default_ident = Ident::new(&"int_n", Span::mixed_site());
            let body = &block.stmts;
            let wrapped_ident = Ident::new(wrapped_name.as_str(), Span::mixed_site());
            // Rename routine to wrap it
            let wrapped = quote! {
                #[no_mangle]
                #[link_section = ".int"]
                pub fn #wrapped_ident () {
                    #(#body)*
                }
            };

            // Define wrapper assembly
            let wrapper = format!(
                "pushad
                call {}
                popad
                iretd",
                wrapped_name
            );

            let wrapper_ident = Ident::new(&name, Span::mixed_site());

            let wrapper = quote! {
                #[naked]
                pub fn #wrapper_ident () {
                    unsafe {
                        asm!(
                            #wrapper
                        , options(noreturn))
                    }
                }
            };

            let stream = quote! {
                #wrapped
                #wrapper
            };
            stream.into()
        }
    }
}

#[proc_macro_attribute]
pub fn interrupt_default(
    args: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let item_2 = TokenStream::from(item.clone());
    let default = parse_macro_input!(item as syn::ItemFn);
    let ItemFn {
        attrs,
        vis,
        sig,
        block,
    } = default;
    // Compute default body
    let default_body = block.stmts;
    // We check which interrupts are implemented in handlers.rs
    let span = proc_macro::Span::call_site();
    // List of all already implemented interrupt to prevent overwriting them
    let mut int_defined = Vec::new();
    let source = span.source_file();
    let path = source.path();
    let file = fs::read_to_string(&path).unwrap();
    let code = parse_file(&file).unwrap();
    for item in code.items {
        match item {
            Item::Fn(f) => {
                if f.sig.ident != sig.ident {
                    let title = f.sig.ident.to_string();
                    // Ignore naked wrappers
                    if title.find("naked").is_none() {
                        let int_number: usize = usize::from_str_radix(
                            title.split("x").collect::<Vec<&str>>().get(1).unwrap(),
                            16,
                        )
                        .unwrap();
                        // We compute interrupts number and append it to the int_defined list
                        int_defined.push(int_number);
                    }
                }
            }
            _ => {}
        }
    }

    let mut default_interrupts = Vec::new();

    // Auto implement other interrupts with default template and a wrapper
    for i in 0..256 {
        if !int_defined.contains(&i) {
            let name = format!("_wrapped_int0x{:x}", i);
            let n = i as u32;
            let ident = Ident::new(name.as_str(), Span::mixed_site());
            let section = String::from(".int");
            let default_int = quote! {
                #[no_mangle]
                #[link_section = #section]
                pub fn #ident (){
                    let int_code : u32 = #n;
                    #(#default_body)*
                }
            };
            let naked_name = format!("_int0x{:x}", i);
            let naked_ident = Ident::new(naked_name.as_str(), Span::mixed_site());
            let wrapper = format!(
                "pushad
                call {}
                popad
                iretd",
                name
            );
            let naked_int = quote! {
                #[link_section = #section]
                #[naked]
                pub fn #naked_ident() {
                    unsafe {asm!(
                        #wrapper
                    , options(noreturn))}
                }
            };
            default_interrupts.push(naked_int);
            default_interrupts.push(default_int);
        }
    }

    let stream = quote! {
        #(#default_interrupts)*
    };

    stream.into()
}