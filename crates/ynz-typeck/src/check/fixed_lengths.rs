//! The checker's side of `fixed<T, N>` in function signatures (v0.3-M9 C1h, card #433).
//!
//! `crate::fixed_length` holds the pure parts (reading a written `fixed<…>`, the specialised
//! versions a program needs). This file holds what needs the checker's state: resolving a written
//! `fixed<…>` with its diagnostics, binding the lengths of a call, and working out a function's
//! returned length from its `return` statements.
//!
//! A child module of `check`, so it reaches the checker's private fields. The diagnostics are
//! rendered by the builders in `check.rs` (`fixed_length_*_diag`): the registry-parity test reads
//! that file for the `registry_diag(...)` calls.

use ynz_ast::nodes::{FunctionDecl, Type as AstType};
use ynz_diagnostics::SourceSpan;

use super::{
    fixed_field_length_missing_diag, fixed_length_mismatch_diag,
    fixed_length_missing_in_export_diag, fixed_length_not_allowed_here_diag,
    fixed_length_not_usable_diag, fixed_return_length_unknown_diag,
    fixed_return_lengths_differ_diag, render_ast_type, Checker,
};
use crate::fixed_length::{
    apply_length_bindings, fixed_type, length_parameters, read_fixed_arguments, FixedArguments,
    FixedCall, WrittenLength, MAX_FIXED_LENGTH,
};
use crate::types::{type_name, FixedLength, Type};

/// One `return` of a function whose return type is a `fixed` without a written length.
#[derive(Clone, Debug)]
pub(super) struct ReturnedLength {
    pub function: String,
    pub length: FixedLength,
    pub span: SourceSpan,
}

fn items(count: usize) -> String {
    if count == 1 {
        "1 item".to_string()
    } else {
        format!("{count} items")
    }
}

fn length_text(length: &FixedLength) -> String {
    match length {
        FixedLength::Known(count) => items(*count),
        FixedLength::Unknown | FixedLength::OfParam { .. } => {
            "a list of unknown length".to_string()
        }
    }
}

impl<'b> Checker<'b> {
    /// `ast_type_to_type` for the type of a function parameter or return value: the one place a
    /// written length is allowed, and only as the whole type. O(size of the type).
    pub(super) fn ast_type_to_type_in_signature(&mut self, ast_ty: &AstType) -> Type {
        self.in_signature_resolution = true;
        self.signature_top_pending = true;
        let resolved = self.ast_type_to_type(ast_ty);
        self.in_signature_resolution = false;
        self.signature_top_pending = false;
        resolved
    }

    /// The type of parameter `index` of function `function`: `declared` (resolved from the
    /// source) with the length the signature table holds, which is the one answer to "how long is
    /// this list" (`OfParam` for an unsized parameter). O(1).
    pub(super) fn parameter_type_from_signature(
        &self,
        function: &str,
        index: usize,
        declared: Type,
    ) -> Type {
        if !matches!(declared, Type::BuiltinFixed { .. }) {
            return declared;
        }
        match self
            .sig_table
            .fns
            .get(function)
            .and_then(|signature| signature.params.get(index))
        {
            Some((_, signature_type @ Type::BuiltinFixed { .. })) => signature_type.clone(),
            _ => declared,
        }
    }

    /// The problems with the arguments of a written `fixed<…>` that make it unusable anywhere: an
    /// argument after the length, a length past the largest, a length that is not a whole number.
    /// O(arguments).
    pub(super) fn report_unusable_fixed_arguments(
        &mut self,
        written: &FixedArguments<'_>,
        written_text: &str,
        element_text: &str,
        span: &SourceSpan,
    ) {
        for _surplus in &written.surplus {
            self.diags.push(fixed_length_not_usable_diag(
                span.clone(),
                written_text,
                "`fixed` takes an element type and a length, and no more",
                &format!(
                    "Write the element type and, if you want one, a length: `fixed<{element_text}, 4>`. To keep values of different types together, declare a shape with a named field for each"
                ),
            ));
        }
        match &written.length {
            WrittenLength::Absent | WrittenLength::Number(_) => {}
            WrittenLength::TooLarge { value, .. } => {
                self.diags.push(fixed_length_not_usable_diag(
                    span.clone(),
                    written_text,
                    &format!(
                        "{value} is more than the {MAX_FIXED_LENGTH} items a `fixed` list can hold"
                    ),
                    &format!("Use an `array<{element_text}>`, which has no fixed length"),
                ));
            }
            WrittenLength::NotANumber(argument) => {
                self.diags.push(fixed_length_not_usable_diag(
                    span.clone(),
                    written_text,
                    &format!("`{}` is not a whole number", render_ast_type(argument)),
                    &format!(
                        "Write a whole number, such as `fixed<{element_text}, 4>`, or leave the length out: `fixed<{element_text}>`. To keep values of different types together, declare a shape with a named field for each"
                    ),
                ));
            }
        }
    }

    /// A written `fixed<…>`: the element, the length, and a diagnostic for every problem with
    /// either. `whole` is the type as written (for the messages); `at_signature_top` is true when
    /// it is the whole type of a parameter or return value. O(size of the arguments).
    pub(super) fn resolve_written_fixed(
        &mut self,
        whole: &AstType,
        args: &[AstType],
        span: &SourceSpan,
        at_signature_top: bool,
    ) -> Type {
        let written_text = render_ast_type(whole);
        let written = read_fixed_arguments(args);
        let element = match written.element {
            Some(element_ast) => self.ast_type_to_type(element_ast),
            None => {
                self.report_missing_type_arguments("fixed", 1, span);
                Type::Error
            }
        };
        let element_text = match written.element {
            Some(element_ast) => render_ast_type(element_ast),
            None => "int".to_string(),
        };
        self.report_unusable_fixed_arguments(&written, &written_text, &element_text, span);
        match &written.length {
            WrittenLength::Absent => {}
            WrittenLength::Number(_) => {
                if !at_signature_top {
                    let place = self.fixed_length_place();
                    let alternative = if self.in_signature_resolution {
                        format!(
                            "Write the length on the whole parameter or return type, or leave it out: `fixed<{element_text}>`"
                        )
                    } else {
                        format!(
                            "Leave the length out: `fixed<{element_text}>`. The list you assign gives it its length"
                        )
                    };
                    self.diags.push(fixed_length_not_allowed_here_diag(
                        span.clone(),
                        &written_text,
                        place,
                        &alternative,
                    ));
                }
            }
            WrittenLength::TooLarge { .. } | WrittenLength::NotANumber(_) => {}
        }
        fixed_type(element, &written.length)
    }

    /// Where a length that is not allowed was written, for the message.
    fn fixed_length_place(&self) -> &'static str {
        if self.in_signature_resolution {
            "inside another type"
        } else {
            self.fixed_length_site
        }
    }

    /// A length written on a generic that takes none (`array<int, 4>`): reported, and dropped
    /// from the arguments the caller resolves.
    pub(super) fn report_length_on_other_generic(
        &mut self,
        whole: &AstType,
        name: &str,
        args: &[AstType],
        span: &SourceSpan,
    ) {
        let without_length: Vec<String> = args
            .iter()
            .filter(|argument| !matches!(argument, AstType::Length { .. }))
            .map(render_ast_type)
            .collect();
        self.diags.push(fixed_length_not_allowed_here_diag(
            span.clone(),
            &render_ast_type(whole),
            &format!("on `{name}`, which takes none"),
            &format!(
                "Leave the length out: `{name}<{}>`",
                without_length.join(", ")
            ),
        ));
    }

    /// The type of the field `field` of `shape`: `fixed<T, N>` is allowed as the whole type (the
    /// shape holds the list inside itself, so its length is part of the shape's size), and a
    /// `fixed<T>` without a length cannot be laid out. A length written anywhere deeper in the type
    /// is refused. O(size of the type).
    pub(super) fn check_shape_field_type(&mut self, shape: &str, field: &str, ast_ty: &AstType) {
        let AstType::Generic {
            name, args, span, ..
        } = ast_ty
        else {
            self.report_lengths_in_nested_field_type(ast_ty);
            return;
        };
        if name != "fixed" {
            self.report_lengths_in_nested_field_type(ast_ty);
            return;
        }
        let written = read_fixed_arguments(args);
        let written_text = render_ast_type(ast_ty);
        let element_text = written
            .element
            .map(render_ast_type)
            .unwrap_or_else(|| "int".to_string());
        self.report_unusable_fixed_arguments(&written, &written_text, &element_text, span);
        if written.length == WrittenLength::Absent {
            self.diags.push(fixed_field_length_missing_diag(
                span.clone(),
                shape,
                field,
                &written_text,
                &element_text,
            ));
        }
        if let Some(element) = written.element {
            self.report_lengths_in_nested_field_type(element);
        }
    }

    /// A length written inside the type of a shape field, where none is allowed (everything but
    /// the whole field type). Walks the whole written type. O(size of the type).
    fn report_lengths_in_nested_field_type(&mut self, ast_ty: &AstType) {
        match ast_ty {
            AstType::Generic {
                name, args, span, ..
            } => {
                if args
                    .iter()
                    .any(|argument| matches!(argument, AstType::Length { .. }))
                {
                    if name == "fixed" {
                        let element_text = read_fixed_arguments(args)
                            .element
                            .map(render_ast_type)
                            .unwrap_or_else(|| "int".to_string());
                        self.diags.push(fixed_length_not_allowed_here_diag(
                            span.clone(),
                            &render_ast_type(ast_ty),
                            "inside another type",
                            &format!(
                                "Write the length on the whole type of the field, or leave it out: `fixed<{element_text}>`"
                            ),
                        ));
                    } else {
                        self.report_length_on_other_generic(ast_ty, name, args, span);
                    }
                }
                for argument in args {
                    self.report_lengths_in_nested_field_type(argument);
                }
            }
            AstType::Maybe { inner, .. } | AstType::ErrorCapable { inner, .. } => {
                self.report_lengths_in_nested_field_type(inner);
            }
            AstType::Sensitive(inner) => self.report_lengths_in_nested_field_type(inner),
            AstType::Union { variants, .. } => {
                for variant in variants {
                    self.report_lengths_in_nested_field_type(variant);
                }
            }
            AstType::AnonShape { fields, .. } => {
                let anonymous_shape = render_ast_type(ast_ty);
                for field in fields {
                    self.check_shape_field_type(&anonymous_shape, &field.name, &field.ty);
                }
            }
            _ => {}
        }
    }

    /// The return type of a call to `name`: what the previous pass worked out for a function whose
    /// return type is a `fixed` without a written length, else the signature's. O(1).
    pub(super) fn seeded_return_type(&self, name: &str, signature_return: &Type) -> Type {
        match self.inferred_returns.get(name) {
            Some(worked_out) => worked_out.clone(),
            None => signature_return.clone(),
        }
    }

    /// The lengths a call passes for the callee's length parameters, with the call's
    /// diagnostics: an argument of another length than a written one, or of unknown length.
    /// Records the call for the specialised versions (`FixedInstances`) and returns the type the
    /// call has (`ret` with the callee's length parameters replaced by what this call passes).
    ///
    /// `arguments[i]` is `(text of the argument, its type, its span)` for `params[i]`.
    /// O(parameters).
    pub(super) fn bind_call_lengths(
        &mut self,
        callee: &str,
        call_key: (usize, usize),
        params: &[(String, Type)],
        arguments: &[(String, Type, SourceSpan)],
        ret: Type,
    ) -> Type {
        let parameters = length_parameters(params);
        let mut bindings: Vec<FixedLength> = Vec::new();
        for ((parameter, declared), (value, actual, argument_span)) in
            params.iter().zip(arguments.iter())
        {
            let Type::BuiltinFixed {
                elem,
                size: declared_size,
            } = declared
            else {
                continue;
            };
            let actual_size = match actual {
                Type::BuiltinFixed { size, .. } => size.clone(),
                _ => FixedLength::Unknown,
            };
            match declared_size {
                FixedLength::OfParam { .. } => bindings.push(actual_size),
                FixedLength::Known(expected) => {
                    if matches!(actual, Type::Error) || !matches!(actual, Type::BuiltinFixed { .. })
                    {
                        continue;
                    }
                    let element = type_name(elem);
                    match actual_size {
                        FixedLength::Known(count) if count == *expected => {}
                        FixedLength::Known(count) => {
                            self.diags.push(fixed_length_mismatch_diag(
                                argument_span.clone(),
                                &format!(
                                    "`{value}` holds {}, but `{callee}` takes exactly {} for `{parameter}`",
                                    items(count),
                                    items(*expected)
                                ),
                                &format!(
                                    "Pass a list of {}, or change `{parameter}` to `fixed<{element}>` so `{callee}` takes a list of any length",
                                    items(*expected)
                                ),
                                &element,
                                *expected,
                            ));
                        }
                        FixedLength::Unknown | FixedLength::OfParam { .. } => {
                            self.diags.push(fixed_length_mismatch_diag(
                                argument_span.clone(),
                                &format!(
                                    "The number of items in `{value}` is not known here, so `{callee}` cannot check it holds the {} it takes for `{parameter}`",
                                    items(*expected)
                                ),
                                &format!(
                                    "Pass a `fixed<{element}>` list written out in this function, so its length is known"
                                ),
                                &element,
                                *expected,
                            ));
                        }
                    }
                }
                FixedLength::Unknown => {}
            }
        }
        if parameters.is_empty() {
            return ret;
        }
        self.fixed_calls.insert(
            call_key,
            FixedCall {
                caller: self.current_fn_name.clone(),
                callee: callee.to_string(),
                lengths: bindings.clone(),
            },
        );
        apply_length_bindings(&ret, &parameters, &bindings)
    }

    /// One `return` of the function being checked, whose return type is `declared`: a written
    /// length is checked against it; an unwritten one is collected so the function's length can be
    /// worked out when its body is done (`finish_function_returns`). O(1).
    pub(super) fn record_returned_length(
        &mut self,
        declared: &Type,
        returned: &Type,
        value_span: &SourceSpan,
    ) {
        let (
            Type::BuiltinFixed {
                elem,
                size: declared_size,
            },
            Type::BuiltinFixed {
                size: returned_size,
                ..
            },
        ) = (declared, returned)
        else {
            return;
        };
        let function = self.current_fn_name.clone();
        match declared_size {
            FixedLength::Known(expected) => {
                let element = type_name(elem);
                match returned_size {
                    FixedLength::Known(count) if count == expected => {}
                    FixedLength::Known(count) => {
                        self.diags.push(fixed_length_mismatch_diag(
                            value_span.clone(),
                            &format!(
                                "`{function}` is declared to return {}, but this `return` gives {}",
                                items(*expected),
                                items(*count)
                            ),
                            &format!(
                                "Return a list of {}, or change the return type to `fixed<{element}, {count}>`",
                                items(*expected)
                            ),
                            &element,
                            *expected,
                        ));
                    }
                    FixedLength::Unknown | FixedLength::OfParam { .. } => {
                        self.diags.push(fixed_length_mismatch_diag(
                            value_span.clone(),
                            &format!(
                                "The number of items in this `return` is not known here, so `{function}` cannot check it gives the {} its return type says",
                                items(*expected)
                            ),
                            &format!(
                                "Return a `fixed<{element}>` list written out in this function, so its length is known"
                            ),
                            &element,
                            *expected,
                        ));
                    }
                }
            }
            FixedLength::Unknown => {
                self.returned_lengths.push(ReturnedLength {
                    function,
                    length: returned_size.clone(),
                    span: value_span.clone(),
                });
            }
            FixedLength::OfParam { .. } => {}
        }
    }

    /// The end of a function whose return type is a `fixed` without a written length: its length
    /// is the one every `return` gives. Recorded in `inferred_return_out` for the next pass (and
    /// for codegen); a function whose `return` statements disagree, or give a list of unknown
    /// length, gets a diagnostic. O(returns of the function).
    pub(super) fn finish_function_returns(&mut self, function: &FunctionDecl, declared: &Type) {
        let Type::BuiltinFixed {
            elem,
            size: FixedLength::Unknown,
        } = declared
        else {
            return;
        };
        let returns: Vec<ReturnedLength> = self
            .returned_lengths
            .iter()
            .filter(|returned| returned.function == function.name)
            .cloned()
            .collect();
        self.returned_lengths
            .retain(|returned| returned.function != function.name);
        let Some(first) = returns.first() else {
            return;
        };
        let element = type_name(elem);
        if let Some(unknown) = returns
            .iter()
            .find(|returned| matches!(returned.length, FixedLength::Unknown))
        {
            self.diags.push(fixed_return_length_unknown_diag(
                unknown.span.clone(),
                &function.name,
                "a `return` gives a list whose length is not known, for example one that comes from a shape field, from `filter`, or from a function whose own length cannot be worked out",
                &element,
            ));
            return;
        }
        let different = returns
            .iter()
            .find(|returned| returned.length != first.length);
        if let Some(different) = different {
            let mut distinct: Vec<String> = Vec::new();
            for returned in &returns {
                let text = length_text(&returned.length);
                if !distinct.contains(&text) {
                    distinct.push(text);
                }
            }
            self.diags.push(fixed_return_lengths_differ_diag(
                different.span.clone(),
                &function.name,
                &distinct.join(" and "),
                &length_text(&first.length),
            ));
            return;
        }
        self.inferred_return_out.insert(
            function.name.clone(),
            Type::BuiltinFixed {
                elem: elem.clone(),
                size: first.length.clone(),
            },
        );
    }

    /// An `export function` must write the length of every `fixed` it takes or returns: another
    /// file sees only the signature. O(parameters).
    pub(super) fn check_exported_fixed_lengths(&mut self, function: &FunctionDecl) {
        if !function.is_exported {
            return;
        }
        let mut positions: Vec<(String, &AstType)> = function
            .params
            .iter()
            .map(|parameter| (format!("parameter `{}`", parameter.name), &parameter.ty))
            .collect();
        positions.push(("return type".to_string(), &function.return_type));
        for (part, ast_ty) in positions {
            let AstType::Generic {
                name, args, span, ..
            } = ast_ty
            else {
                continue;
            };
            if name != "fixed" {
                continue;
            }
            let written = read_fixed_arguments(args);
            if written.length != WrittenLength::Absent {
                continue;
            }
            let element_text = written
                .element
                .map(render_ast_type)
                .unwrap_or_else(|| "int".to_string());
            self.diags.push(fixed_length_missing_in_export_diag(
                span.clone(),
                &function.name,
                &part,
                &render_ast_type(ast_ty),
                &format!("fixed<{element_text}, 4>"),
            ));
        }
    }
}
