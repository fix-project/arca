(** Specifies the names and Wasm signatures of the modeled Fix Shell imports. *)

From Wasm Require Import datatypes host.
From Stdlib Require Import List String.
Import ListNotations.
Local Open Scope string_scope.
Local Open Scope list_scope.

Inductive shell_function :=
| shell_get_type | shell_get_data_type | shell_get_encode_type
| shell_is_tag | shell_handle_bytes | shell_equals | shell_len
| shell_get_tree_element | shell_attach_blob | shell_create_tree
| shell_create_blob_i64 | shell_create_ref | shell_create_tag
| shell_application | shell_selection | shell_identification
| shell_strict | shell_shallow.

Definition shell_function_eq_dec (left right : shell_function) : {left=right}+{left<>right}.
Proof. decide equality. Defined.
#[export] Instance shell_host_functions : host_function_class :=
  {| host_function := shell_function; host_function_eq_dec := shell_function_eq_dec |}.

Definition shell_signature operation :=
  let r := T_ref T_externref in let i := T_num T_i32 in
  match operation with
  | shell_get_type | shell_get_data_type | shell_get_encode_type
  | shell_is_tag => Tf [r] [i]
  | shell_len => Tf [r] [T_num T_i64]
  | shell_equals => Tf [r; r] [i]
  | shell_get_tree_element => Tf [r; i] [r]
  | shell_attach_blob => Tf [r; i] []
  | shell_create_tree | shell_create_tag => Tf [i; i] [r]
  | shell_create_blob_i64 => Tf [T_num T_i64] [r]
  | _ => Tf [r] [r]
  end.

Definition shell_function_imports :=
  [("get_type", shell_get_type); ("get_data_type", shell_get_data_type);
   ("get_encode_type", shell_get_encode_type); ("is_tag", shell_is_tag);
   ("unsound_handle_bytes", shell_handle_bytes); ("equals", shell_equals);
   ("len", shell_len); ("get_tree_element", shell_get_tree_element);
   ("attach_blob", shell_attach_blob); ("create_tree", shell_create_tree);
   ("create_blob_i64", shell_create_blob_i64); ("create_ref", shell_create_ref);
   ("create_tag", shell_create_tag); ("create_application_thunk", shell_application);
   ("create_selection_thunk", shell_selection); ("create_identification_thunk", shell_identification);
   ("create_strict_encode", shell_strict); ("create_shallow_encode", shell_shallow)].
