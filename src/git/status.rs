use super::*;
use crate::i18n::messages;

pub(super) struct Status {
    pub head: Option<String>,
    pub branch: String,
    pub tracking: Option<BranchTracking>,
    pub changes: Arc<[ChangedFile]>,
    pub changes_digest: [u8; 32],
}

pub(super) fn parse(bytes: &[u8], previous: Option<&Repository>) -> Result<Status> {
    let mut records = bytes.split(|b| *b == 0).peekable();
    let mut oid = None;
    let mut branch = None;
    let mut header_len = 0;
    while let Some(header) = records.next_if(|record| record.starts_with(b"# ")) {
        header_len += header.len() + 1;
        if let Some(value) = header.strip_prefix(b"# branch.oid ") {
            oid = Some(value);
        } else if let Some(value) = header.strip_prefix(b"# branch.head ") {
            branch = Some(String::from_utf8_lossy(value).into_owned());
        }
    }
    let oid = oid.context(messages().missing_head())?;
    let branch = branch.context(messages().missing_branch())?;
    let tracking = parse_tracking(bytes);
    use sha2::{Digest, Sha256};
    let changes_digest = Sha256::digest(&bytes[header_len.min(bytes.len())..]).into();
    let changes = if let Some(repo) = previous.filter(|repo| repo.changes_digest == changes_digest)
    {
        repo.changes.clone()
    } else {
        parse_changes(records)?.into()
    };
    Ok(Status {
        head: (oid != b"(initial)").then(|| String::from_utf8_lossy(oid).into_owned()),
        branch,
        tracking,
        changes,
        changes_digest,
    })
}

fn parse_changes<'a>(mut records: impl Iterator<Item = &'a [u8]>) -> Result<Vec<ChangedFile>> {
    let mut changes = Vec::new();
    while let Some(record) = records.next() {
        if record.is_empty() {
            continue;
        }
        if let Some(path) = record.strip_prefix(b"? ") {
            changes.push(ChangedFile {
                status: "??".into(),
                path: String::from_utf8_lossy(path).into_owned(),
                original_path: None,
            });
            continue;
        }
        // Porcelain v2 has a fixed field count per record type. The final
        // field is a literal path, which can contain spaces and newlines.
        let fields = match record[0] {
            b'1' => 9,
            b'2' => 10,
            b'u' => 11,
            _ => bail!(messages().unknown_status()),
        };
        let mut parts = record.splitn(fields, |b| *b == b' ');
        let xy = parts
            .nth(1)
            .filter(|xy| xy.len() == 2)
            .context(messages().incomplete_status())?;
        let path = parts
            .nth(fields - 3)
            .context(messages().incomplete_path())?;
        let original_path = if record[0] == b'2' {
            let original = records
                .next()
                .filter(|p| !p.is_empty())
                .context(messages().incomplete_rename())?;
            Some(String::from_utf8_lossy(original).into_owned())
        } else {
            None
        };
        changes.push(ChangedFile {
            status: std::str::from_utf8(xy)?.replace('.', " "),
            path: String::from_utf8_lossy(path).into_owned(),
            original_path,
        });
    }
    Ok(changes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_including_rename_sources_cannot_be_mistaken_for_headers() {
        let status = parse(
            concat!(
                "# branch.oid abc\0# branch.head main\0# branch.upstream origin/main\0",
                "# branch.ab +3 -2\0",
                "1 .M N... 100644 100644 100644 a b  leading space.txt\0",
                "2 R. N... 100644 100644 100644 a b R100 new name.txt\0# branch.upstream fake\0",
                "u UU N... 100644 100644 100644 100644 a b c 冲突.txt\0",
                "? new\nfile.txt\0",
            )
            .as_bytes(),
            None,
        )
        .unwrap();
        assert_eq!(status.head.as_deref(), Some("abc"));
        assert_eq!(status.branch, "main");
        assert_eq!(
            status.tracking.unwrap(),
            BranchTracking {
                upstream: "origin/main".into(),
                counts: Some((3, 2)),
            }
        );
        let files = status.changes;
        assert_eq!(files.len(), 4);
        assert_eq!(
            (&*files[0].status, &*files[0].path),
            (" M", " leading space.txt")
        );
        assert_eq!((&*files[1].status, &*files[1].path), ("R ", "new name.txt"));
        assert_eq!(
            files[1].original_path.as_deref(),
            Some("# branch.upstream fake")
        );
        assert_eq!((&*files[2].status, &*files[2].path), ("UU", "冲突.txt"));
        assert_eq!(
            (&*files[3].status, &*files[3].path),
            ("??", "new\nfile.txt")
        );
    }
}
