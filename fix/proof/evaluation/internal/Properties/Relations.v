  (** Force can return Data as an Object or as a Ref, while a later consumer chooses how
      to expose it. The relation on replies must therefore compare their underlying
      Objects. The shallow-Encode case likewise moves to the strict request so that both
      sides are compared before lowering to Refs. *)
  Definition relaxed_X (X : handle -> handle -> Prop) h1 h2 :=
    match h1 with
    | Encode (Shallow th) => X (Encode (Strict th)) (lift h2)
    | _ => X (lift h1) (lift h2)
    end.
  (** A Think reply is not necessarily final Data: it can defer work as a Thunk or an
      Encode. The relation needed by the next Force step must account for that deferral.
      In particular, two Encode replies may be justified through related Thunks or through
      already related forces; the subsequent continuation argument needs both
      alternatives. *)
  Definition strengthen (X : handle -> handle -> Prop) h1 h2 :=
    match h1, h2 with
    | Encode e1, Encode e2 =>
        X (Thunk (encode_to_thunk e1)) (Thunk (encode_to_thunk e2)) \/
        rel_opt (relaxed_X X) (force (encode_to_thunk e1)) (force (encode_to_thunk e2))
    | Encode e, Thunk th => X (Thunk (encode_to_thunk e)) (Thunk th)
    | Thunk th, Encode e => X (Thunk th) (Thunk (encode_to_thunk e))
    | _, _ => relaxed_X X h1 h2
    end.
