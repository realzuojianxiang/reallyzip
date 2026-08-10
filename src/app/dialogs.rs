//! 各类对话框：压缩、解压、密码、设置、关于、预览、提示。

use super::*;

impl ReallyZipApp {
    pub(super) fn dialogs(&mut self, ctx: &egui::Context) {
        match self.dlg.clone() {
            Dialog::None => {}
            Dialog::Create => self.create_dialog(ctx),
            Dialog::Extract => self.extract_dialog(ctx),
            Dialog::Password => self.password_dialog(ctx),
            Dialog::Settings => self.settings_dialog(ctx),
            Dialog::About => self.about_dialog(ctx),
        }
    }

    // ------------------------------------------------------------ 压缩

    fn create_dialog(&mut self, ctx: &egui::Context) {
        let mut open = true;
        let mut submit = false;
        let mut cancel = false;

        egui::Window::new(if self.create.append {
            "添加文件到压缩包"
        } else {
            "创建压缩文件"
        })
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(560.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.add_space(2.0);
            ui.label(RichText::new("目标压缩文件").strong());
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.create.dest)
                        .desired_width(430.0)
                        .hint_text("例如 D:\\备份\\资料.zip"),
                );
                if ui.button("浏览…").clicked() {
                    let mut d = rfd::FileDialog::new().add_filter("ZIP 压缩包", &["zip"]);
                    if let Some(parent) = Path::new(&self.create.dest).parent()
                        && parent.exists() {
                            d = d.set_directory(parent);
                        }
                    if let Some(name) = Path::new(&self.create.dest).file_name() {
                        d = d.set_file_name(name.to_string_lossy().to_string());
                    }
                    if let Some(p) = d.save_file() {
                        self.create.dest = p.display().to_string();
                    }
                }
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            egui::Frame::new()
                .fill(theme::SECTION_BG)
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(egui::Margin::same(14))
                .show(ui, |ui| {
                    egui::Grid::new("create_grid")
                .num_columns(2)
                .spacing([16.0, 10.0])
                .show(ui, |ui| {
                    ui.label("压缩方式");
                    egui::ComboBox::from_id_salt("level")
                        .selected_text(self.create.level.label())
                        .width(200.0)
                        .show_ui(ui, |ui| {
                            for l in Level::ALL {
                                ui.selectable_value(&mut self.create.level, l, l.label());
                            }
                        });
                    ui.end_row();

                    ui.label("分卷大小");
                    ui.horizontal(|ui| {
                        let cur = volume::PRESETS[self.create.split_idx.min(volume::PRESETS.len())
                            .min(volume::PRESETS.len() - 1)]
                        .0;
                        let shown = if self.create.split_idx == volume::PRESETS.len() {
                            "自定义"
                        } else {
                            cur
                        };
                        egui::ComboBox::from_id_salt("split")
                            .selected_text(shown)
                            .width(200.0)
                            .show_ui(ui, |ui| {
                                for (i, (name, _)) in volume::PRESETS.iter().enumerate() {
                                    ui.selectable_value(&mut self.create.split_idx, i, *name);
                                }
                                ui.selectable_value(
                                    &mut self.create.split_idx,
                                    volume::PRESETS.len(),
                                    "自定义…",
                                );
                            });
                        if self.create.split_idx == volume::PRESETS.len() {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.create.split_custom_mb)
                                    .desired_width(80.0)
                                    .hint_text("MB"),
                            );
                            ui.label("MB");
                        }
                    });
                    ui.end_row();

                    ui.label("加密");
                    ui.checkbox(&mut self.create.use_password, "为压缩包设置密码（AES-256）");
                    ui.end_row();

                    if self.create.use_password {
                        ui.label("密码");
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.create.password)
                                    .password(!self.create.show_pw)
                                    .desired_width(200.0),
                            );
                            ui.checkbox(&mut self.create.show_pw, "显示");
                        });
                        ui.end_row();

                        ui.label("确认密码");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.create.password2)
                                .password(!self.create.show_pw)
                                .desired_width(200.0),
                        );
                        ui.end_row();
                    }
                });
                });

            if self.create.append {
                ui.add_space(6.0);
                ui.label(
                    RichText::new("追加模式：文件会写入已存在的压缩包，同名条目会被跳过。")
                        .color(theme::TEXT_FAINT)
                        .size(12.0),
                );
            }

            ui.add_space(8.0);
            ui.label(RichText::new(format!("待压缩项（{}）", self.create.sources.len())).strong());
            egui::Frame::new()
                .fill(theme::CARD_BG)
                .stroke(egui::Stroke::new(1.0, theme::BORDER))
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                .max_height(120.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for s in &self.create.sources {
                        ui.horizontal(|ui| {
                            let (_, r) = ui.allocate_space(egui::vec2(15.0, 15.0));
                            icons::paint_in(
                                ui,
                                r,
                                if s.is_dir() {
                                    IconKind::Folder
                                } else {
                                    IconKind::File
                                },
                            );
                            ui.add(
                                egui::Label::new(
                                    RichText::new(s.display().to_string()).size(12.0),
                                )
                                .truncate(),
                            );
                        });
                    }
                });
                });

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add_sized([88.0, 32.0], egui::Button::new("取消"))
                        .clicked()
                    {
                        cancel = true;
                    }
                    let ok = ui.add_sized(
                        [104.0, 32.0],
                        egui::Button::new(RichText::new("开始压缩").strong())
                            .fill(theme::ACCENT)
                            .stroke(egui::Stroke::NONE)
                            .corner_radius(egui::CornerRadius::same(6)),
                    );
                    if ok.clicked() {
                        submit = true;
                    }
                    if ui
                        .add_sized([88.0, 32.0], egui::Button::new("添加文件…"))
                        .clicked()
                        && let Some(mut more) = rfd::FileDialog::new().pick_files()
                    {
                        self.create.sources.append(&mut more);
                    }
                    if ui
                        .add_sized([96.0, 32.0], egui::Button::new("添加文件夹…"))
                        .clicked()
                        && let Some(more) = rfd::FileDialog::new().pick_folder()
                    {
                        self.create.sources.push(more);
                    }
                });
            });
        });

        if !open || cancel {
            self.dlg = Dialog::None;
            return;
        }
        if !submit {
            return;
        }

        // 校验
        let dest = self.create.dest.trim().to_string();
        if dest.is_empty() {
            self.error = Some("请填写目标压缩文件路径".into());
            return;
        }
        let mut dest = PathBuf::from(dest);
        if dest.extension().is_none() {
            dest.set_extension("zip");
        }
        if self.create.use_password {
            if self.create.password.is_empty() {
                self.error = Some("密码不能为空".into());
                return;
            }
            if self.create.password != self.create.password2 {
                self.error = Some("两次输入的密码不一致".into());
                return;
            }
        }
        let split_size = if self.create.split_idx == volume::PRESETS.len() {
            match self.create.split_custom_mb.trim().parse::<f64>() {
                Ok(mb) if mb > 0.0 => (mb * 1024.0 * 1024.0) as u64,
                _ => {
                    self.error = Some("请输入有效的自定义分卷大小（MB）".into());
                    return;
                }
            }
        } else {
            volume::PRESETS[self.create.split_idx].1
        };

        if self.create.append && split_size > 0 {
            self.error = Some("追加模式不支持分卷".into());
            return;
        }
        if !self.create.append && dest.exists() {
            let choice = rfd::MessageDialog::new()
                .set_title("文件已存在")
                .set_description(format!(
                    "{} 已存在。\n\n是：覆盖重建　否：取消",
                    dest.display()
                ))
                .set_level(rfd::MessageLevel::Warning)
                .set_buttons(rfd::MessageButtons::YesNo)
                .show();
            if choice != rfd::MessageDialogResult::Yes {
                return;
            }
        }

        let opt = CreateOptions {
            level: self.create.level,
            password: if self.create.use_password {
                Some(self.create.password.clone())
            } else {
                None
            },
            split_size,
        };
        let sources = self.create.sources.clone();
        let append = self.create.append;
        self.dlg = Dialog::None;
        self.start_create(ctx, sources, dest, opt, append);
    }

    // ------------------------------------------------------------ 解压

    fn extract_dialog(&mut self, ctx: &egui::Context) {
        let mut open = true;
        let mut submit = false;
        let mut cancel = false;

        egui::Window::new("解压文件")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(520.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(RichText::new("目标目录").strong());
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.extract.dest).desired_width(390.0),
                    );
                    if ui.button("浏览…").clicked() {
                        let mut d = rfd::FileDialog::new();
                        let cur = PathBuf::from(&self.extract.dest);
                        if let Some(p) = cur.parent()
                            && p.exists()
                        {
                            d = d.set_directory(p);
                        }
                        if let Some(p) = d.pick_folder() {
                            self.extract.dest = p.display().to_string();
                        }
                    }
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label("解压范围：");
                    ui.label(
                        RichText::new(&self.extract.count_label)
                            .color(theme::ACCENT)
                            .strong(),
                    );
                });

                ui.add_space(6.0);
                ui.checkbox(&mut self.extract.keep_paths, "保留压缩包内的目录结构");

                ui.add_space(6.0);
                ui.label(RichText::new("同名文件处理").strong());
                ui.horizontal(|ui| {
                    for m in [Overwrite::AutoRename, Overwrite::Always, Overwrite::Skip] {
                        ui.radio_value(&mut self.extract.overwrite, m, m.label());
                    }
                });

                if self.extract.needs_pw {
                    ui.add_space(8.0);
                    egui::Frame::new()
                        .fill(theme::LOCK_SOFT)
                        .corner_radius(egui::CornerRadius::same(6))
                        .inner_margin(egui::Margin::symmetric(10, 6))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let (_, r) = ui.allocate_space(egui::vec2(15.0, 15.0));
                                icons::paint_in(ui, r, IconKind::Locked);
                                ui.label(RichText::new("压缩包已加密").color(theme::LOCK).strong().size(12.5));
                            });
                        });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label("密码");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.extract.password)
                                .password(!self.extract.show_pw)
                                .desired_width(220.0),
                        );
                        ui.checkbox(&mut self.extract.show_pw, "显示");
                    });
                }

                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add_sized([88.0, 32.0], egui::Button::new("取消"))
                            .clicked()
                        {
                            cancel = true;
                        }
                        if ui
                            .add_sized(
                                [104.0, 32.0],
                                egui::Button::new(RichText::new("开始解压").strong())
                                    .fill(theme::ACCENT)
                                    .stroke(egui::Stroke::NONE)
                                    .corner_radius(egui::CornerRadius::same(6)),
                            )
                            .clicked()
                        {
                            submit = true;
                        }
                    });
                });
            });

        if !open || cancel {
            self.dlg = Dialog::None;
            return;
        }
        if !submit {
            return;
        }

        let dest = self.extract.dest.trim();
        if dest.is_empty() {
            self.error = Some("请填写解压目标目录".into());
            return;
        }
        if self.extract.needs_pw && self.extract.password.is_empty() {
            self.error = Some("该压缩包已加密，请输入密码".into());
            return;
        }

        let pw = if self.extract.password.is_empty() {
            None
        } else {
            Some(self.extract.password.clone())
        };
        if pw.is_some() {
            self.password = pw.clone();
        }

        let opt = ExtractOptions {
            dest: PathBuf::from(dest),
            selection: self.extract.selection.clone(),
            password: pw,
            keep_paths: self.extract.keep_paths,
            overwrite: self.extract.overwrite,
        };
        let archive_path = self.extract.archive.clone();
        self.dlg = Dialog::None;
        self.start_extract(ctx, archive_path, opt);
    }

    // ------------------------------------------------------------ 密码

    fn password_dialog(&mut self, ctx: &egui::Context) {
        let mut open = true;
        let mut submit = false;
        let mut cancel = false;

        egui::Window::new("需要密码")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(380.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let (_, r) = ui.allocate_space(egui::vec2(22.0, 22.0));
                    icons::paint_in(ui, r, IconKind::Locked);
                    ui.label(RichText::new(&self.pw_hint).size(13.0));
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.label("密码");
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.pw_input)
                            .password(!self.pw_show)
                            .desired_width(200.0),
                    );
                    resp.request_focus();
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        submit = true;
                    }
                    ui.checkbox(&mut self.pw_show, "显示");
                });
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add_sized([80.0, 32.0], egui::Button::new("取消")).clicked() {
                            cancel = true;
                        }
                        if ui
                            .add_sized(
                                [88.0, 32.0],
                                egui::Button::new(RichText::new("确定").strong())
                                    .fill(theme::ACCENT)
                                    .stroke(egui::Stroke::NONE)
                                    .corner_radius(egui::CornerRadius::same(6)),
                            )
                            .clicked()
                        {
                            submit = true;
                        }
                    });
                });
            });

        if !open || cancel {
            self.dlg = Dialog::None;
            self.pending_after_pw = None;
            return;
        }
        if submit {
            if self.pw_input.is_empty() {
                self.error = Some("密码不能为空".into());
                return;
            }
            // 能拿到压缩包路径时，先校验密码；错误则留在对话框，避免错误密码被缓存导致永久锁死。
            if let Some(ap) = &self.pw_archive
                && !archive::verify_password(ap, &self.pw_input) {
                    self.error = Some("密码错误，请重新输入".into());
                    return;
                }
            self.password = Some(self.pw_input.clone());
            self.dlg = Dialog::None;
            if let Some(a) = self.pending_after_pw.take() {
                self.pending.push(a);
            }
        }
    }

    // ------------------------------------------------------------ 设置

    fn settings_dialog(&mut self, ctx: &egui::Context) {
        let mut open = true;
        egui::Window::new("设置")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(480.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(RichText::new("资源管理器右键菜单").strong().size(14.0));
                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        "注册后，在文件或文件夹上点右键即可直接压缩；\
                         在 .zip 上点右键可直接解压。\n\
                         Windows 11 中可能需要点“显示更多选项”才能看到。",
                    )
                    .color(theme::TEXT_DIM)
                    .size(12.0),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let status = if self.shell_registered {
                        RichText::new("● 已注册").color(theme::OK_GREEN).strong()
                    } else {
                        RichText::new("○ 未注册").color(theme::TEXT_DIM)
                    };
                    ui.label(status);
                    ui.add_space(12.0);
                    if ui.button("注册右键菜单").clicked() {
                        match shell::register() {
                            Ok(_) => {
                                self.shell_registered = shell::is_registered();
                                self.info = Some("右键菜单已注册成功。".into());
                            }
                            Err(e) => self.error = Some(format!("注册失败：{e}")),
                        }
                    }
                    if ui.button("取消注册").clicked() {
                        match shell::unregister() {
                            Ok(_) => {
                                self.shell_registered = shell::is_registered();
                                self.info = Some("右键菜单已移除。".into());
                            }
                            Err(e) => self.error = Some(format!("移除失败：{e}")),
                        }
                    }
                });

                ui.add_space(14.0);
                ui.separator();
                ui.add_space(6.0);
                ui.label(RichText::new("临时文件").strong().size(14.0));
                let tmp = std::env::temp_dir().join("reallyzip");
                ui.label(
                    RichText::new(format!("目录：{}", tmp.display()))
                        .color(theme::TEXT_DIM)
                        .size(12.0),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("清理临时文件").clicked() {
                        let mut n = 0;
                        if let Ok(rd) = std::fs::read_dir(&tmp) {
                            for e in rd.flatten() {
                                let p = e.path();
                                let ok = if p.is_dir() {
                                    std::fs::remove_dir_all(&p).is_ok()
                                } else {
                                    std::fs::remove_file(&p).is_ok()
                                };
                                if ok {
                                    n += 1;
                                }
                            }
                        }
                        // 顺带清掉历史版本（RustRAR / RustZip）遗留的临时目录
                        for old in ["rustrar", "rustzip"] {
                            let legacy = std::env::temp_dir().join(old);
                            if legacy.is_dir() && std::fs::remove_dir_all(&legacy).is_ok() {
                                n += 1;
                            }
                        }
                        self.info = Some(format!("已清理 {n} 个临时项。"));
                    }
                    if ui.button("打开临时目录").clicked() {
                        let _ = std::fs::create_dir_all(&tmp);
                        util::open_with_system(&tmp);
                    }
                });

                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add_sized([80.0, 28.0], egui::Button::new("关闭")).clicked() {
                            self.dlg = Dialog::None;
                        }
                    });
                });
            });
        if !open {
            self.dlg = Dialog::None;
        }
    }

    // ------------------------------------------------------------ 关于

    fn about_dialog(&mut self, ctx: &egui::Context) {
        let mut open = true;
        egui::Window::new("关于 ReallyZip")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(440.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(theme::ACCENT_SOFTER)
                    .corner_radius(egui::CornerRadius::same(10))
                    .inner_margin(egui::Margin::same(14))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (_, r) = ui.allocate_space(egui::vec2(44.0, 44.0));
                            icons::paint_in(ui, r, IconKind::Archive);
                            ui.vertical(|ui| {
                                ui.add_space(4.0);
                                ui.label(RichText::new("ReallyZip").size(20.0).strong().color(theme::ACCENT_DARK));
                                ui.label(
                                    RichText::new(format!("版本 {}", env!("CARGO_PKG_VERSION")))
                                        .color(theme::TEXT_DIM),
                                );
                            });
                        });
                    });
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(6.0);
                ui.label("功能一览：");
                for line in [
                    "· 创建 / 解压 ZIP，六档压缩级别",
                    "· AES-256 密码加密与解密",
                    "· 分卷压缩（.001 / .002 …），打开时自动合并",
                    "· 不解压直接浏览压缩包目录树、预览文本文件",
                    "· 向已有压缩包追加文件、从压缩包中删除条目",
                    "· CRC32 完整性测试",
                    "· Windows 资源管理器右键菜单集成",
                    "· 拖拽文件到窗口即可压缩",
                ] {
                    ui.label(RichText::new(line).size(12.5));
                }
                ui.add_space(10.0);
                ui.label(
                    RichText::new("使用 Rust + egui 构建。")
                        .color(theme::TEXT_DIM)
                        .size(12.0),
                );
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add_sized([80.0, 28.0], egui::Button::new("确定")).clicked() {
                            self.dlg = Dialog::None;
                        }
                    });
                });
            });
        if !open {
            self.dlg = Dialog::None;
        }
    }

    // ------------------------------------------------------- 提示 / 预览

    pub(super) fn message_windows(&mut self, ctx: &egui::Context) {
        if let Some(err) = self.error.clone() {
            let mut open = true;
            egui::Window::new("操作未完成")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .default_width(430.0)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, -40.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        let (_, r) = ui.allocate_space(egui::vec2(20.0, 20.0));
                        paint_error_icon(ui, r);
                        ui.vertical(|ui| {
                            ui.label(RichText::new("出错了").strong().size(14.0));
                            ui.add_space(2.0);
                            egui::Frame::new()
                                .fill(theme::DANGER_SOFT)
                                .corner_radius(egui::CornerRadius::same(6))
                                .inner_margin(egui::Margin::symmetric(10, 6))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(err)
                                            .color(theme::TEXT_DIM)
                                            .size(12.5)
                                            .line_height(Some(18.0)),
                                    );
                                });
                        });
                    });
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.add_sized([88.0, 30.0], egui::Button::new("知道了")).clicked() {
                                self.error = None;
                            }
                        });
                    });
                });
            if !open {
                self.error = None;
            }
        }

        if let Some(info) = self.info.clone() {
            let mut open = true;
            egui::Window::new("完成")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .default_width(440.0)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, -40.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        let (_, r) = ui.allocate_space(egui::vec2(20.0, 20.0));
                        paint_check_icon(ui, r);
                        ui.vertical(|ui| {
                            ui.label(RichText::new("操作完成").strong().size(14.0));
                            ui.add_space(2.0);
                            ui.label(
                                RichText::new(info)
                                    .color(theme::TEXT_DIM)
                                    .size(12.5)
                                    .line_height(Some(18.0)),
                            );
                        });
                    });
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.add_sized([88.0, 30.0], egui::Button::new("确定")).clicked() {
                                self.info = None;
                            }
                        });
                    });
                });
            if !open {
                self.info = None;
            }
        }

        let mut close_preview = false;
        if let Some(pv) = &self.preview {
            let mut open = true;
            egui::Window::new(format!("查看：{}", pv.title))
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .default_size([720.0, 500.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("大小 {}", util::format_size(pv.size)))
                                .color(theme::TEXT_DIM)
                                .size(12.0),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("用系统默认程序打开").clicked() {
                                util::open_with_system(&pv.path);
                            }
                        });
                    });
                    ui.separator();
                    match &pv.text {
                        Some(text) => {
                            egui::ScrollArea::both().auto_shrink([false, false]).show(
                                ui,
                                |ui| {
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(text)
                                                .family(egui::FontFamily::Monospace)
                                                .size(12.5),
                                        )
                                        .wrap_mode(egui::TextWrapMode::Extend),
                                    );
                                },
                            );
                        }
                        None => {
                            ui.add_space(30.0);
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    RichText::new("这是二进制文件，无法以文本方式预览。")
                                        .color(theme::TEXT_DIM),
                                );
                                ui.add_space(8.0);
                                if ui.button("用系统默认程序打开").clicked() {
                                    util::open_with_system(&pv.path);
                                }
                            });
                        }
                    }
                });
            if !open {
                close_preview = true;
            }
        }
        if close_preview {
            self.preview = None;
        }
    }
}

/// 错误提示窗口里的感叹号图标。
fn paint_error_icon(ui: &egui::Ui, rect: egui::Rect) {
    let p = ui.painter();
    let c = rect.center();
    let r = rect.width() * 0.5;
    p.circle_filled(c, r, theme::DANGER);
    p.circle_filled(
        egui::Pos2::new(c.x, c.y - r * 0.28),
        r * 0.14,
        egui::Color32::WHITE,
    );
    p.line_segment(
        [
            egui::Pos2::new(c.x, c.y - r * 0.05),
            egui::Pos2::new(c.x, c.y + r * 0.42),
        ],
        egui::Stroke::new(r * 0.22, egui::Color32::WHITE),
    );
}

/// 成功提示窗口里的对勾图标。
fn paint_check_icon(ui: &egui::Ui, rect: egui::Rect) {
    let p = ui.painter();
    let c = rect.center();
    let r = rect.width() * 0.5;
    p.circle_filled(c, r, theme::OK_GREEN);
    let s = r * 0.42;
    p.add(egui::Shape::convex_polygon(
        vec![
            egui::Pos2::new(c.x - s, c.y - s * 0.1),
            egui::Pos2::new(c.x - s * 0.2, c.y + s * 0.6),
            egui::Pos2::new(c.x + s, c.y - s * 0.5),
            egui::Pos2::new(c.x + s * 0.6, c.y - s * 0.7),
            egui::Pos2::new(c.x - s * 0.35, c.y + s * 0.25),
        ],
        egui::Color32::WHITE,
        egui::Stroke::NONE,
    ));
}
