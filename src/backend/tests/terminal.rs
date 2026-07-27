    #[test]
    fn terminal_protocol_parses_input_and_clamps_resize_messages() {
        assert_eq!(
            parse_terminal_client_message(r#"{"type":"input","data":"printf ok\n"}"#),
            Some(TerminalClientMessage::Input("printf ok\n".to_string()))
        );
        assert_eq!(
            parse_terminal_client_message(r#"{"type":"resize","cols":9999,"rows":0}"#),
            Some(TerminalClientMessage::Resize(TerminalDimensions {
                cols: 500,
                rows: 2,
            }))
        );
        assert_eq!(parse_terminal_client_message("not-json"), None);
        assert_eq!(
            parse_terminal_client_message(r#"{"type":"other","data":"x"}"#),
            None
        );
    }

    #[test]
    fn terminal_origin_policy_allows_same_origin_and_native_clients() {
        let mut same_origin = HeaderMap::new();
        same_origin.insert(header::HOST, HeaderValue::from_static("yahu.example:443"));
        same_origin.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://yahu.example:443"),
        );
        assert!(websocket_origin_allowed(&same_origin));

        let mut cross_origin = same_origin.clone();
        cross_origin.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://attacker.example"),
        );
        assert!(!websocket_origin_allowed(&cross_origin));

        let mut native_client = HeaderMap::new();
        native_client.insert(header::HOST, HeaderValue::from_static("127.0.0.1:9642"));
        assert!(websocket_origin_allowed(&native_client));
    }

    #[test]
    fn terminal_home_uses_the_current_process_user_directory() {
        assert_eq!(
            terminal_home_dir(Some(std::ffi::OsString::from("/home/test-user"))),
            PathBuf::from("/home/test-user")
        );
        assert_eq!(terminal_home_dir(None), PathBuf::from("/"));
    }

    #[test]
    fn terminal_cwd_is_limited_to_existing_workspace_directories() {
        let workspace = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let nested = workspace.path().join("nested folder");
        std::fs::create_dir(&nested).unwrap();
        std::fs::write(workspace.path().join("file.txt"), "x").unwrap();

        assert_eq!(
            terminal_working_dir(workspace.path(), Some("nested folder"), home.path()).unwrap(),
            nested
        );
        assert_eq!(
            terminal_working_dir(workspace.path(), None, home.path()).unwrap(),
            home.path()
        );
        assert!(terminal_working_dir(workspace.path(), Some("../outside"), home.path()).is_err());
        assert!(terminal_working_dir(workspace.path(), Some("file.txt"), home.path()).is_err());
    }

    #[test]
    fn terminal_pty_runs_a_shell_command_and_resizes() {
        let home = tempfile::tempdir().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        let mut terminal = spawn_terminal_process(
            Path::new("/bin/bash"),
            cwd.path(),
            home.path(),
            TerminalDimensions { cols: 80, rows: 24 },
        )
        .unwrap();

        resize_terminal_process(
            terminal.master.as_ref(),
            TerminalDimensions {
                cols: 132,
                rows: 41,
            },
        )
        .unwrap();
        terminal
            .writer
            .write_all(b"printf 'YAHU_PTY_OK:%s:%s:%s:%s\\n' \"$COLUMNS\" \"$LINES\" \"$PWD\" \"$HOME\"; exit\n")
            .unwrap();
        terminal.writer.flush().unwrap();

        let mut output = String::new();
        terminal.reader.read_to_string(&mut output).unwrap();
        let status = terminal.child.wait().unwrap();

        assert!(status.success());
        let expected = format!(
            "YAHU_PTY_OK:132:41:{}:{}",
            cwd.path().display(),
            home.path().display()
        );
        assert!(output.contains(&expected), "{output:?}");
    }

    #[test]
    fn terminal_route_is_authenticated_and_discoverable() {
        let backend_source = include_str!("../mod.rs");
        assert!(backend_source.contains(".route(\"/terminal/ws\", get(web_terminal_websocket))"));
        let auth_source = include_str!("../auth.rs");
        assert!(auth_source.contains("path.starts_with(\"/terminal/\")"));
    }
