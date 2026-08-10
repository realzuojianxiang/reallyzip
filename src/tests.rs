//! 无界面引擎测试：覆盖压缩/解压/加密/分卷/删除/校验等核心逻辑。

#![cfg(test)]

use crate::archive::*;
use crate::task::Reporter;
use crate::volume::*;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

fn rep() -> Reporter {
    Reporter::new(egui::Context::default())
}

fn tmp_root() -> PathBuf {
    let p = std::env::temp_dir().join(format!("reallyzip_test_{}", std::process::id()));
    fs::create_dir_all(&p).unwrap();
    p
}

fn write(p: &Path, content: &str) {
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(p, content).unwrap();
}

/// 校验压缩包内容是否与源目录完全一致（含目录结构）。
fn assert_roundtrip(root: &Path, zip: &Path, password: Option<&str>) {
    let out = tmp_root().join("extracted");
    let _ = fs::remove_dir_all(&out);
    let opt = ExtractOptions {
        dest: out.clone(),
        selection: None,
        password: password.map(str::to_string),
        keep_paths: true,
        overwrite: Overwrite::Always,
    };
    extract(zip, &opt, &rep()).expect("extract 失败");

    // 直接按压缩包内部路径逐条比对，去掉首个分量（源目录名）定位原文件
    let entries = read_entries(zip).expect("read_entries 失败");
    for e in entries {
        if e.is_dir {
            continue;
        }
        let extracted = out.join(&e.path);
        let content = fs::read(&extracted)
            .unwrap_or_else(|_| panic!("解压后缺少文件：{}", e.path));
        let rel: PathBuf = e.path.split('/').skip(1).collect();
        let original = root.join(&rel);
        let want = fs::read(&original)
            .unwrap_or_else(|_| panic!("源文件不存在：{}", original.display()));
        assert_eq!(content, want, "内容不一致：{}", e.path);
    }
}

#[test]
fn create_and_extract_roundtrip() {
    let root = tmp_root().join("rt");
    let _ = fs::remove_dir_all(&root);
    write(&root.join("hello.txt"), "Hello, ReallyZip!");
    write(&root.join("docs/note.md"), "# 标题\n正文内容");
    write(&root.join("data/big.bin"), &"X".repeat(20_000));

    let zip = tmp_root().join("rt.zip");
    let _ = fs::remove_file(&zip);
    let opt = CreateOptions {
        level: Level::Normal,
        password: None,
        split_size: 0,
    };
    let msg = create(&[root.clone()], &zip, &opt, &rep()).expect("create 失败");
    assert!(zip.exists(), "{msg}");

    let ar = open(&zip, &rep()).expect("open 失败");
    assert_eq!(ar.file_count(), 3, "文件数应为 3");

    assert_roundtrip(&root, &zip, None);
}

#[test]
fn level_store_is_bigger_than_best() {
    let root = tmp_root().join("lv");
    let _ = fs::remove_dir_all(&root);
    write(&root.join("f.txt"), &"a".repeat(50_000));

    let store = tmp_root().join("store.zip");
    let best = tmp_root().join("best.zip");
    create(
        &[root.clone()],
        &store,
        &CreateOptions {
            level: Level::Store,
            ..Default::default()
        },
        &rep(),
    )
    .unwrap();
    create(
        &[root.clone()],
        &best,
        &CreateOptions {
            level: Level::Best,
            ..Default::default()
        },
        &rep(),
    )
    .unwrap();
    let s = fs::metadata(&store).unwrap().len();
    let b = fs::metadata(&best).unwrap().len();
    assert!(b < s, "最大压缩应小于存储：{b} < {s}");
}

#[test]
fn aes_password_protection() {
    let root = tmp_root().join("pw");
    let _ = fs::remove_dir_all(&root);
    write(&root.join("secret.txt"), "顶部机密");

    let zip = tmp_root().join("pw.zip");
    let _ = fs::remove_file(&zip);
    create(
        &[root.clone()],
        &zip,
        &CreateOptions {
            level: Level::Normal,
            password: Some("s3cret".into()),
            split_size: 0,
        },
        &rep(),
    )
    .unwrap();

    let ar = open(&zip, &rep()).unwrap();
    assert!(ar.has_encrypted, "应标记为已加密");

    // verify_password：正确密码通过、错误密码拒绝
    assert!(
        verify_password(&zip, "s3cret"),
        "正确密码应通过校验"
    );
    assert!(
        !verify_password(&zip, "wrong"),
        "错误密码应通过校验被拒绝"
    );
    assert!(
        !verify_password(&zip, ""),
        "空密码应通过校验被拒绝"
    );

    // 错误密码解压应失败
    let bad = tmp_root().join("bad");
    let err = extract(
        &zip,
        &ExtractOptions {
            dest: bad,
            selection: None,
            password: Some("wrong".into()),
            keep_paths: true,
            overwrite: Overwrite::Always,
        },
        &rep(),
    );
    assert!(err.is_err(), "错误密码应当解压失败");

    // 正确密码可解压且内容一致
    assert_roundtrip(&root, &zip, Some("s3cret"));
}

#[test]
fn volume_split_and_merge() {
    let root = tmp_root().join("vol");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    // 伪随机数据，几乎不可压缩，确保分卷真正生效
    let blob: Vec<u8> = (0..70_000u32)
        .map(|i| ((i.wrapping_mul(2654435761) >> 22) ^ i) as u8)
        .collect();
    fs::write(&root.join("a.bin"), &blob).unwrap();
    fs::write(&root.join("b.bin"), &blob).unwrap();

    let base = tmp_root().join("archive.zip");
    create(
        &[root.clone()],
        &base,
        &CreateOptions {
            level: Level::Normal,
            password: None,
            split_size: 30 * 1024, // 30KB 每卷
        },
        &rep(),
    )
    .unwrap();

    let first_part = PathBuf::from(format!("{}.001", base.display()));
    assert!(first_part.exists(), "应存在首个分卷 {}", first_part.display());

    let volumes = collect_volumes(&first_part).expect("collect_volumes 失败");
    assert!(volumes.len() >= 2, "应至少切出 2 个分卷，实际 {}", volumes.len());
    assert!(is_volume_part(&first_part), "first_part 应被识别为分卷");

    // 合并后再打开，应能正常读取并解压
    let merged = merge_volumes(&volumes, &rep()).expect("merge_volumes 失败");
    let ar = open(&merged, &rep()).expect("合并后打开失败");
    assert_eq!(ar.file_count(), 2, "合并后应读取到 2 个文件");
    assert_roundtrip(&root, &merged, None);
    let _ = fs::remove_file(&merged);
}

#[test]
fn delete_entries_works() {
    let root = tmp_root().join("del");
    let _ = fs::remove_dir_all(&root);
    write(&root.join("keep.txt"), "保留");
    write(&root.join("drop.txt"), "删除");
    write(&root.join("docs/x.txt"), "也保留");

    let zip = tmp_root().join("del.zip");
    create(
        &[root.clone()],
        &zip,
        &CreateOptions {
            level: Level::Normal,
            ..Default::default()
        },
        &rep(),
    )
    .unwrap();
    let before = open(&zip, &rep()).unwrap().file_count();
    assert_eq!(before, 3);

    let mut targets = HashSet::new();
    targets.insert("del/drop.txt".to_string());
    let msg = delete_entries(&zip, &targets, &rep()).expect("delete 失败");
    assert!(msg.contains("删除"), "删除消息应包含『删除』：{msg}");
    let after = open(&zip, &rep()).unwrap().file_count();
    assert_eq!(after, 2, "删除后应剩 2 个文件，实际 {after}");
}

#[test]
fn test_crc_ok() {
    let root = tmp_root().join("crc");
    let _ = fs::remove_dir_all(&root);
    write(&root.join("ok.txt"), "CRC 校验内容");

    let zip = tmp_root().join("crc.zip");
    create(
        &[root.clone()],
        &zip,
        &CreateOptions {
            level: Level::Normal,
            ..Default::default()
        },
        &rep(),
    )
    .unwrap();
    let res = test(&zip, None, &rep());
    assert!(res.is_ok(), "CRC 校验应当通过：{res:?}");
}
