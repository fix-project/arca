(** Specifies the names and Wasm signatures of the modeled Fix Shell imports. *)

From Wasm Require Import datatypes host.
From Stdlib Require Import List String NArith.
Import ListNotations.
Local Open Scope string_scope.
Local Open Scope list_scope.

Definition native_table_limit : N := 134217728%N.

Inductive shell_function :=
| shell_get_type | shell_get_data_type | shell_get_encode_type
| shell_is_tag | shell_same_handle_bytes | shell_equals | shell_len
| shell_get_tree_element | shell_attach_blob | shell_read_blob | shell_create_tree
| shell_create_blob_i64 | shell_create_ref | shell_create_tag
| shell_application | shell_selection | shell_identification
| shell_strict | shell_shallow | shell_create_tag_unchecked | shell_get_thunk_type
| shell_clone | shell_close.

Definition shell_function_eq_dec (left right : shell_function) : {left=right}+{left<>right}.
Proof. decide equality. Defined.
#[export] Instance shell_host_functions : host_function_class :=
  {| host_function := shell_function; host_function_eq_dec := shell_function_eq_dec |}.

Definition shell_signature operation :=
  let i := T_num T_i32 in
  match operation with
  | shell_len => Tf [i] [T_num T_i64]
  | shell_same_handle_bytes | shell_equals | shell_get_tree_element
  | shell_attach_blob | shell_create_tree => Tf [i; i] [i]
  | shell_read_blob => Tf [i; i; i; i] [i]
  | shell_create_blob_i64 => Tf [T_num T_i64] [i]
  | shell_close => Tf [i] []
  | _ => Tf [i] [i]
  end.

Definition shell_function_imports :=
  [("get_type", shell_get_type); ("get_data_type", shell_get_data_type);
   ("get_encode_type", shell_get_encode_type); ("is_tag", shell_is_tag);
   ("same_handle_bytes", shell_same_handle_bytes); ("equals", shell_equals);
   ("len", shell_len); ("get_tree_element", shell_get_tree_element);
   ("access_blob", shell_read_blob);
   ("create_tree", shell_create_tree);
   ("create_blob_i64", shell_create_blob_i64); ("create_ref", shell_create_ref);
   ("create_tag", shell_create_tag); ("create_application_thunk", shell_application);
   ("create_selection_thunk", shell_selection); ("create_identification_thunk", shell_identification);
   ("create_strict_encode", shell_strict); ("create_shallow_encode", shell_shallow);
   ("create_tag_unchecked", shell_create_tag_unchecked); ("get_thunk_type", shell_get_thunk_type);
   ("close", shell_close); ("clone", shell_clone)].
