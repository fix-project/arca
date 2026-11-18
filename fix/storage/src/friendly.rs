use alloc::{collections::BTreeSet, format, string::String};
use core::ops::Bound::{Excluded, Unbounded};
use fixhandle::*;

#[derive(Debug, Default)]
pub(crate) struct Names(BTreeSet<[u8; 31]>);

impl Names {
    pub fn extend(&mut self, names: impl IntoIterator<Item = [u8; 32]>) {
        for name in names {
            if let Ok(handle) = Handle::parse_ref(&name)
                && matches!(handle.name(), Name::Canonical(_))
            {
                self.0
                    .insert(name[..31].try_into().expect("fixed name width"));
            }
        }
    }

    fn abbreviated(&self, value: Handle) -> String {
        let Name::Canonical(_) = value.name() else {
            return format!("{value}");
        };
        let hash: &[u8; 31] = value.as_bytes()[..31].try_into().expect("fixed name width");
        let text = format!("{value}");
        let neighbors = [
            self.0.range(..*hash).next_back(),
            self.0.range((Excluded(*hash), Unbounded)).next(),
        ];
        let shared = neighbors.into_iter().flatten().map(|other| {
            let mut shared = 0;
            for (a, b) in hash.iter().zip(other) {
                let same = (a ^ b).leading_zeros() as usize / 4;
                shared += same;
                if same != 2 {
                    break;
                }
            }
            shared
        });
        let length = shared.max().map_or(8, |shared| 8.max(shared + 1)).min(62);
        format!("{}.{:02x}", &text[..length], value.as_bytes()[31])
    }

    /// Resolve names from labels and resident coupon metadata without materializing data.
    pub fn friendly_name(
        &mut self,
        value: Handle,
        labels: &[([u8; 32], String)],
        read: impl Fn(Tree) -> Option<alloc::boxed::Box<[Handle]>>,
    ) -> String {
        self.extend(
            core::iter::once(*value.as_bytes()).chain(labels.iter().map(|(name, _)| *name)),
        );
        self.named(value, labels, &read, true)
            .unwrap_or_else(|| self.abbreviated(value))
    }

    fn named(
        &self,
        value: Handle,
        labels: &[([u8; 32], String)],
        read: &impl Fn(Tree) -> Option<alloc::boxed::Box<[Handle]>>,
        coupons: bool,
    ) -> Option<String> {
        if let Some((_, label)) = labels.iter().find(|(name, _)| name == value.as_bytes()) {
            return Some(label.clone());
        }
        match value.view() {
            HandleView::Encode(encode) => {
                let (prefix, thunk) = match encode.view() {
                    EncodeView::Strict(thunk) => ("*", thunk),
                    EncodeView::Shallow(thunk) => ("+", thunk),
                };
                self.named(thunk.into(), labels, read, coupons)
                    .map(|name| format!("{prefix}{name}"))
            }
            HandleView::Thunk(thunk) => {
                let (prefix, reference) = match thunk.view() {
                    ThunkView::Identification(reference) => ("'", reference),
                    ThunkView::Application(reference) => ("#", reference.into()),
                    ThunkView::Selection(reference) => {
                        let fields = read(reference.object_descriptor())?;
                        if !matches!(fields.len(), 2 | 3) {
                            return None;
                        }
                        let arguments = fields
                            .iter()
                            .map(|value| {
                                self.named(*value, labels, read, false)
                                    .unwrap_or_else(|| self.abbreviated(*value))
                            })
                            .collect::<alloc::vec::Vec<_>>();
                        return Some(format!("[{}]", arguments.join(" ")));
                    }
                };
                let operand = labels
                    .iter()
                    .find(|(name, _)| name == reference.as_handle().as_bytes())
                    .map(|(_, label)| label.clone())
                    .or_else(|| {
                        self.named(reference.object_descriptor().into(), labels, read, coupons)
                    })?;
                Some(format!("{prefix}{operand}"))
            }
            HandleView::Ref(reference) => self
                .named(reference.object_descriptor().into(), labels, read, coupons)
                .map(|name| format!("&{name}")),
            HandleView::Object(object) => {
                if value.is_value()
                    && let Some((_, label)) = labels.iter().find(|(name, _)| {
                        name[..31] == value.as_bytes()[..31]
                            && name[31] & 0xe0 == value.as_bytes()[31] & 0xe0
                            && name[31] & 0x1f == 0x10
                    })
                {
                    return Some(format!("*'{label}"));
                }
                if let ObjectView::Blob(blob) = object.view()
                    && let Some(bytes) = blob.literal_bytes()
                {
                    if let Ok(text) = core::str::from_utf8(bytes)
                        && text.chars().all(|character| !character.is_control())
                    {
                        return Some(format!("{text:?}"));
                    }
                    if [1, 2, 4, 8, 16].contains(&bytes.len()) {
                        let mut number = [0; 16];
                        number[..bytes.len()].copy_from_slice(bytes);
                        return Some(format!(
                            "{}u{}",
                            u128::from_le_bytes(number),
                            bytes.len() * 8
                        ));
                    }
                }
                if !coupons {
                    return None;
                }
                let ObjectView::Tree(tree) = object.view() else {
                    return None;
                };
                if !tree.is_tag() || tree.len() != 2 {
                    return None;
                }
                let fields = read(*tree)?;
                let reference = TreeRef::try_from(fields[1]).ok()?;
                if reference.len() != 3 {
                    return None;
                }
                let claim = read(reference.object_descriptor())?;
                let operation = Blob::try_from(claim[0]).ok()?;
                let opcode = u64::from_le_bytes(operation.literal_bytes()?.try_into().ok()?);
                let operation = match opcode {
                    0 => "Eq",
                    1 => "Eval",
                    2 => "Apply",
                    3 => "Force",
                    4 => "Think",
                    6 => "Select",
                    _ => return None,
                };
                let input = self.named(claim[1], labels, read, false);
                let result = self.named(claim[2], labels, read, false);
                if input.is_none() && result.is_none() {
                    return None;
                }
                Some(format!(
                    "{operation}({}, {})",
                    input.unwrap_or_else(|| self.abbreviated(claim[1])),
                    result.unwrap_or_else(|| self.abbreviated(claim[2]))
                ))
            }
        }
    }
}
