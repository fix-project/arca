use crate::commands::CliError;
use fixhandle::{Blob, EncodeView, Handle, HandleView, Name, ObjectView, ThunkView, Tree};
use fixrepository::Repository;
use fixstorage::{Storage, repository::RepositoryStorage};
use std::fmt::Write;

fn representation(value: Handle) -> String {
    match value.view() {
        HandleView::Object(_) => "Object".into(),
        HandleView::Ref(_) => "Ref".into(),
        HandleView::Thunk(thunk) => format!(
            "Thunk ({:?})",
            match thunk.view() {
                ThunkView::Application(_) => fix::ThunkType::Application,
                ThunkView::Identification(_) => fix::ThunkType::Identification,
                ThunkView::Selection(_) => fix::ThunkType::Selection,
            }
        ),
        HandleView::Encode(encode) => match encode.view() {
            EncodeView::Strict(thunk) => {
                format!("Strict Encode of {}", representation(thunk.into()))
            }
            EncodeView::Shallow(thunk) => {
                format!("Shallow Encode of {}", representation(thunk.into()))
            }
        },
    }
}

pub fn describe(repository: &Repository, name: &str, abbreviate: bool) -> Result<String, CliError> {
    let storage = RepositoryStorage::new(Repository::open(repository.path())?.unsigned());
    let inspection = storage.repository();
    let names = repository
        .labels()?
        .into_iter()
        .map(|label| inspection.label_name(&label).map(|name| (name, label)))
        .collect::<Result<Vec<_>, _>>()?;
    let display = |value: Handle| {
        if abbreviate {
            storage.friendly_name(value, &names)
        } else {
            format!("{value}")
        }
    };
    let bytes = inspection.resolve_name(name)?;
    let view = Handle::parse_ref(&bytes)?;
    if !view.is_canonical() {
        return Err(fixrepository::RepositoryError::NonCanonical.into());
    }
    let value = unsafe { view.to_owned() };
    let object = value.object_descriptor();
    let mut output = format!("Name: {value}\nRepresentation: {}\n", representation(value));
    let kind = match object.view() {
        ObjectView::Blob(_) => "Blob",
        ObjectView::Tree(tree) if tree.is_tag() => "Tag",
        ObjectView::Tree(_) => "Tree",
    };
    writeln!(output, "Data: {kind}, {} bytes", object.byte_len()).unwrap();
    if let ObjectView::Tree(tree) = object.view() {
        writeln!(output, "Fields: {}", tree.len()).unwrap();
    }
    writeln!(
        output,
        "Comparable: {}\nValue: {}",
        value.is_comparable(),
        value.is_value()
    )
    .unwrap();
    match repository.has_backing(object) {
        Ok(present) => writeln!(
            output,
            "Local backing: {}",
            if present { "present" } else { "absent" }
        )
        .unwrap(),
        Err(error) => writeln!(output, "Local backing: cannot validate ({error})").unwrap(),
    }
    let mut labels = Vec::new();
    for (_, label) in &names {
        let candidate = inspection.read_label(label)?;
        if same_subject(candidate, value) {
            labels.push(label.clone());
        }
    }
    writeln!(
        output,
        "Labels: {}",
        if labels.is_empty() {
            "none".into()
        } else {
            labels.join(", ")
        }
    )
    .unwrap();
    if let Some(metadata) = repository.get_todo(object.into_ref())? {
        writeln!(output, "Acquisition metadata: {}", display(metadata)).unwrap();
        if let Ok(tree) = Tree::try_from(metadata) {
            for field in repository.read_tree(tree)?.iter() {
                if let Ok(blob) = Blob::try_from(*field) {
                    let bytes = repository.read_blob(blob)?;
                    writeln!(output, "  {}", String::from_utf8_lossy(&bytes)).unwrap();
                } else {
                    writeln!(output, "  {}", display(*field)).unwrap();
                }
            }
        }
    } else {
        writeln!(output, "Acquisition metadata: none").unwrap();
    }
    if let ObjectView::Tree(tree) = object.view()
        && tree.is_tag()
    {
        let blobs = repository.certificate_blobs(object.as_handle().as_bytes())?;
        writeln!(output, "SSH signatures: {}", blobs.len()).unwrap();
        if !blobs.is_empty() {
            let policy = repository.policy()?;
            for certificate in repository.available_certificates(object.as_handle().as_bytes())? {
                writeln!(
                    output,
                    "  {} ({})",
                    certificate
                        .signer()
                        .map_err(fixrepository::RepositoryError::from)?,
                    if certificate
                        .verify(object.as_handle().as_bytes(), &policy)
                        .is_ok()
                    {
                        "trusted"
                    } else {
                        "untrusted"
                    }
                )
                .unwrap();
            }
        }
        match repository.read_tree(*tree) {
            Ok(fields) => {
                if let Some(author) = fields.first() {
                    writeln!(output, "Author: {}", display(*author)).unwrap();
                }
                for (index, field) in fields.iter().enumerate().skip(1) {
                    writeln!(output, "Field {index}: {}", display(*field)).unwrap();
                }
            }
            Err(error) => writeln!(output, "Tag fields: cannot read ({error})").unwrap(),
        }
    }
    let mut coupons = Vec::new();
    let mut certificates = Vec::new();
    let mut tags = Vec::new();
    let mut unreadable = Vec::new();
    for candidate in repository.inventory()? {
        let Ok(tag) = Tree::try_from(candidate) else {
            continue;
        };
        if !tag.is_tag() {
            continue;
        }
        let fields = match inspection.read_tree(tag) {
            Ok(fields) => fields,
            Err(error) => {
                unreadable.push(format!("  {}: {error}", display(candidate)));
                continue;
            }
        };
        if fields.is_empty() {
            continue;
        }
        let direct = fields
            .iter()
            .skip(1)
            .any(|field| same_subject(*field, value));
        let coupon = coupon_description(inspection, &fields, value, &display)?;
        if !direct && coupon.is_none() {
            continue;
        }
        let signature = match repository.lookup(candidate.as_bytes()) {
            Ok(_) => "accepted".into(),
            Err(error) => format!("not accepted ({error})"),
        };
        let details = format!(
            "    Tag: {}\n    Author: {}\n    Signature: {signature}",
            display(candidate),
            display(fields[0])
        );
        if let Some(coupon) = coupon {
            coupons.push(format!("{coupon}\n{details}"));
        } else if matches!(fields.as_ref(), [_, _, kind] if literal(kind) == Some(b"Runnable")) {
            certificates.push(format!("  Runnable: {}\n{details}", display(fields[1])));
        } else {
            let mut description = format!(
                "  {}\n    Author: {}\n    Signature: {signature}",
                display(candidate),
                display(fields[0])
            );
            for (index, field) in fields.iter().enumerate().skip(1) {
                write!(description, "\n    Field {index}: {}", display(*field)).unwrap();
                if !abbreviate && let Some(bytes) = literal(field) {
                    write!(description, " ({:?})", String::from_utf8_lossy(bytes)).unwrap();
                }
            }
            tags.push(description);
        }
    }
    for (title, entries) in [
        ("Coupons", coupons),
        ("Recognized certificates", certificates),
        ("Other tags", tags),
        ("Unreadable tags", unreadable),
    ] {
        writeln!(output, "{title}: {}", entries.len()).unwrap();
        for entry in entries {
            writeln!(output, "{entry}").unwrap();
        }
    }
    Ok(output)
}

fn same_subject(left: Handle, right: Handle) -> bool {
    if left.same_encoding(&right) {
        return true;
    }
    match (crate::reference(left), crate::reference(right)) {
        (Ok(left), Ok(right)) => left.as_handle().same_encoding(right.as_handle()),
        _ => false,
    }
}

fn operation_name(operation: Handle) -> Option<&'static str> {
    let bytes = <[u8; 8]>::try_from(literal(&operation)?).ok()?;
    match u64::from_le_bytes(bytes) {
        0 => Some("Equivalent"),
        1 => Some("Evaluate"),
        2 => Some("Apply"),
        3 => Some("Force"),
        4 => Some("Think"),
        6 => Some("Select"),
        _ => None,
    }
}

fn literal(value: &Handle) -> Option<&[u8]> {
    Blob::try_from(*value).ok()?;
    match value.name() {
        Name::Literal(bytes) => Some(bytes),
        _ => None,
    }
}

fn coupon_description(
    repository: &Repository,
    fields: &[Handle],
    value: Handle,
    display: &impl Fn(Handle) -> String,
) -> Result<Option<String>, CliError> {
    let [_, claim] = fields else {
        return Ok(None);
    };
    let Ok(reference) = crate::reference(*claim) else {
        return Ok(None);
    };
    let Ok(tree) = Tree::try_from(*reference.object_descriptor().as_handle()) else {
        return Ok(None);
    };
    if !repository.has_backing(tree.into())? {
        return Ok(None);
    }
    let fields = repository.read_tree(tree)?;
    let [operation, input, result] = fields.as_ref() else {
        return Ok(None);
    };
    if !same_subject(*input, value) && !same_subject(*result, value) {
        return Ok(None);
    }
    Ok(operation_name(*operation)
        .map(|operation| format!("  {operation}: {} -> {}", display(*input), display(*result))))
}
