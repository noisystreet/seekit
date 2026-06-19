//! 交互式 REPL 模式
//!
//! 支持连续搜索、翻页、打开浏览器、获取内容等操作。
//! 使用 rustyline 提供行编辑和历史记录功能。

use std::time::Instant;

use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Config, EditMode, Editor, Helper};

use crate::cli::Cli;
use crate::engine::{EngineConfig, EngineType, SearchEngine};
use crate::{engine, fetcher};

/// REPL 助手（无自定义补全行为，但 rustyline 需要这个 trait）
struct ReplHelper;

impl Completer for ReplHelper {
    type Candidate = Pair;
}

impl Hinter for ReplHelper {
    type Hint = String;
}

impl Highlighter for ReplHelper {}

impl Validator for ReplHelper {}

impl Helper for ReplHelper {}

/// REPL 命令
#[derive(Debug)]
enum ReplCommand {
    /// 执行搜索
    Search(String),
    /// 下一页
    NextPage,
    /// 上一页
    PrevPage,
    /// 打开结果（o <N>）
    Open(usize),
    /// 获取内容（f <N>）
    Fetch(usize),
    /// 显示帮助
    Help,
    /// 退出
    Quit,
}

/// REPL 运行状态
struct ReplState {
    /// 当前搜索的 CLI 参数快照
    cli: Cli,
    /// 当前页结果
    results: Vec<engine::SearchResult>,
    /// 当前页码
    page: u32,
    /// 最后一页查询词
    last_query: String,
}

impl ReplState {
    fn new(cli: Cli) -> Self {
        Self {
            cli,
            results: Vec::new(),
            page: 1,
            last_query: String::new(),
        }
    }
}

/// 解析用户输入为 REPL 命令
fn parse_command(input: &str) -> ReplCommand {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return ReplCommand::Search(String::new());
    }

    if is_quit(trimmed) {
        return ReplCommand::Quit;
    }

    if is_help(trimmed) {
        return ReplCommand::Help;
    }

    if let Some(cmd) = try_page_cmd(trimmed) {
        return cmd;
    }

    if let Some(n) = try_number_cmd(trimmed, &["o ", "open "]) {
        return ReplCommand::Open(n);
    }

    if let Some(n) = try_number_cmd(trimmed, &["f ", "fetch "]) {
        return ReplCommand::Fetch(n);
    }

    ReplCommand::Search(trimmed.to_string())
}

fn is_quit(s: &str) -> bool {
    matches!(s, "q" | "quit" | "exit")
}

fn is_help(s: &str) -> bool {
    matches!(s, "h" | "help" | "?")
}

fn try_page_cmd(s: &str) -> Option<ReplCommand> {
    if s == "n" || s == "next" {
        return Some(ReplCommand::NextPage);
    }
    if s == "p" || s == "prev" {
        return Some(ReplCommand::PrevPage);
    }
    None
}

fn try_number_cmd(s: &str, prefixes: &[&str]) -> Option<usize> {
    for prefix in prefixes {
        if let Some(rest) = s.strip_prefix(prefix) {
            if let Ok(n) = rest.trim().parse::<usize>() {
                return Some(n);
            }
        }
    }
    None
}

/// 执行搜索
async fn do_search(state: &mut ReplState, query: &str, page: u32) -> anyhow::Result<()> {
    let start = Instant::now();

    // 更新 CLI 参数
    state.cli.query = Some(query.to_string());
    state.cli.page = page;

    let engine_config = EngineConfig {
        max_results: state.cli.max_results,
        timeout_secs: state.cli.timeout,
        safe_search: !state.cli.no_safe,
        lang: if state.cli.lang.is_empty() {
            None
        } else {
            Some(state.cli.lang.clone())
        },
        page,
    };

    let engine = create_repl_engine(&state.cli)?;
    let engine_name = engine.name();
    let results = engine.search(query, &engine_config).await?;

    state.results = results;
    state.last_query = query.to_string();
    state.page = page;

    let elapsed = start.elapsed();
    print_results(&state.results, engine_name, query, elapsed);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    // ── ReplState ─────────────────────────────────────────

    #[test]
    fn test_repl_state_new() {
        let cli = Cli::parse_from(&["seekit", "-i"]);
        let state = ReplState::new(cli);
        assert!(state.results.is_empty());
        assert_eq!(state.page, 1);
        assert!(state.last_query.is_empty());
    }

    // ── parse_command ─────────────────────────────────────

    #[test]
    fn test_parse_command_search_query() {
        match parse_command("rust programming") {
            ReplCommand::Search(q) => assert_eq!(q, "rust programming"),
            _ => panic!("expected Search"),
        }
    }

    #[test]
    fn test_parse_command_search_with_special_chars() {
        match parse_command("C++ tutorial 2024") {
            ReplCommand::Search(q) => assert_eq!(q, "C++ tutorial 2024"),
            _ => panic!("expected Search"),
        }
    }

    #[test]
    fn test_parse_command_quit() {
        for cmd in &["q", "quit", "exit"] {
            assert!(
                matches!(parse_command(cmd), ReplCommand::Quit),
                "failed for {cmd}"
            );
        }
    }

    #[test]
    fn test_parse_command_help() {
        for cmd in &["h", "help", "?"] {
            assert!(
                matches!(parse_command(cmd), ReplCommand::Help),
                "failed for {cmd}"
            );
        }
    }

    #[test]
    fn test_parse_command_next_page() {
        for cmd in &["n", "next"] {
            assert!(
                matches!(parse_command(cmd), ReplCommand::NextPage),
                "failed for {cmd}"
            );
        }
    }

    #[test]
    fn test_parse_command_prev_page() {
        for cmd in &["p", "prev"] {
            assert!(
                matches!(parse_command(cmd), ReplCommand::PrevPage),
                "failed for {cmd}"
            );
        }
    }

    #[test]
    fn test_parse_command_open() {
        for (input, expected) in &[
            ("o 1", 1usize),
            ("o 42", 42),
            ("open 3", 3),
            ("open 99", 99),
        ] {
            match parse_command(input) {
                ReplCommand::Open(n) => assert_eq!(n, *expected, "failed for {input}"),
                _ => panic!("expected Open for {input}"),
            }
        }
    }

    #[test]
    fn test_parse_command_fetch() {
        for (input, expected) in &[
            ("f 1", 1usize),
            ("f 10", 10),
            ("fetch 3", 3),
            ("fetch 100", 100),
        ] {
            match parse_command(input) {
                ReplCommand::Fetch(n) => assert_eq!(n, *expected, "failed for {input}"),
                _ => panic!("expected Fetch for {input}"),
            }
        }
    }

    #[test]
    fn test_parse_command_trailing_spaces() {
        match parse_command("  hello world  ") {
            ReplCommand::Search(q) => assert_eq!(q, "hello world"),
            _ => panic!("expected Search"),
        }
    }

    #[test]
    fn test_parse_command_open_no_number_is_search() {
        match parse_command("o") {
            ReplCommand::Search(q) => assert_eq!(q, "o"),
            other => panic!("expected Search(\"o\"), got {:?}", other),
        }
    }

    #[test]
    fn test_parse_command_empty_input() {
        match parse_command("") {
            ReplCommand::Search(q) => assert!(q.is_empty()),
            _ => panic!("expected Search for empty input"),
        }
    }

    #[test]
    fn test_parse_command_whitespace() {
        match parse_command("  ") {
            ReplCommand::Search(q) => assert!(q.is_empty()),
            _ => panic!("expected Search for whitespace"),
        }
    }

    // ── is_quit ────────────────────────────────────────────

    #[test]
    fn test_is_quit_true() {
        assert!(is_quit("q"));
        assert!(is_quit("quit"));
        assert!(is_quit("exit"));
    }

    #[test]
    fn test_is_quit_false() {
        assert!(!is_quit("query"));
        assert!(!is_quit(""));
    }

    // ── is_help ────────────────────────────────────────────

    #[test]
    fn test_is_help_true() {
        assert!(is_help("h"));
        assert!(is_help("help"));
        assert!(is_help("?"));
    }

    // ── try_page_cmd ───────────────────────────────────────

    #[test]
    fn test_try_page_cmd_next() {
        assert!(matches!(try_page_cmd("n"), Some(ReplCommand::NextPage)));
        assert!(matches!(try_page_cmd("next"), Some(ReplCommand::NextPage)));
    }

    #[test]
    fn test_try_page_cmd_prev() {
        assert!(matches!(try_page_cmd("p"), Some(ReplCommand::PrevPage)));
        assert!(matches!(try_page_cmd("prev"), Some(ReplCommand::PrevPage)));
    }

    #[test]
    fn test_try_page_cmd_other() {
        assert!(try_page_cmd("hello").is_none());
        assert!(try_page_cmd("").is_none());
    }

    // ── try_number_cmd ─────────────────────────────────────

    #[test]
    fn test_try_number_cmd_valid() {
        assert_eq!(try_number_cmd("o 5", &["o ", "open "]), Some(5));
        assert_eq!(try_number_cmd("f 10", &["f ", "fetch "]), Some(10));
        assert_eq!(try_number_cmd("open 42", &["o ", "open "]), Some(42));
    }

    #[test]
    fn test_try_number_cmd_invalid() {
        assert!(try_number_cmd("o abc", &["o ", "open "]).is_none());
        assert!(try_number_cmd("hello", &["o ", "open "]).is_none());
    }

    // ── validate_index ─────────────────────────────────────

    #[test]
    fn test_validate_index_valid() {
        assert!(validate_index(1, 5));
        assert!(validate_index(5, 5));
    }

    #[test]
    fn test_validate_index_zero() {
        assert!(!validate_index(0, 5));
    }

    #[test]
    fn test_validate_index_out_of_range() {
        assert!(!validate_index(6, 5));
    }

    #[test]
    fn test_validate_index_empty_results() {
        assert!(!validate_index(1, 0));
    }

    // ── handle_page_cmd ────────────────────────────────────

    #[test]
    fn test_handle_page_cmd_next_no_query() {
        let cli = Cli::parse_from(&["seekit"]);
        let state = ReplState::new(cli);
        let result = handle_page_cmd(&state, PageDirection::Next);
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_handle_page_cmd_next_with_query() {
        let cli = Cli::parse_from(&["seekit"]);
        let mut state = ReplState::new(cli);
        state.last_query = "rust".to_string();
        state.page = 1;
        let result = handle_page_cmd(&state, PageDirection::Next).unwrap();
        assert_eq!(result, Some(("rust".to_string(), 2)));
    }

    #[test]
    fn test_handle_page_cmd_prev_on_first_page() {
        let cli = Cli::parse_from(&["seekit"]);
        let state = ReplState::new(cli);
        let result = handle_page_cmd(&state, PageDirection::Prev);
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_handle_page_cmd_prev_with_query() {
        let cli = Cli::parse_from(&["seekit"]);
        let mut state = ReplState::new(cli);
        state.last_query = "rust".to_string();
        state.page = 3;
        let result = handle_page_cmd(&state, PageDirection::Prev).unwrap();
        assert_eq!(result, Some(("rust".to_string(), 2)));
    }

    // ── print_help ─────────────────────────────────────────

    #[test]
    fn test_print_help_does_not_panic() {
        print_help();
    }

    // ── open_in_browser ────────────────────────────────────

    #[test]
    fn test_open_in_browser_invalid_url() {
        // 只测试不 panic
        open_in_browser("https://example.com");
    }
}

/// 创建搜索引擎（REPL 版本，不带缓存）
fn create_repl_engine(cli: &Cli) -> anyhow::Result<Box<dyn SearchEngine>> {
    let engine_type: EngineType = cli
        .engine
        .parse()
        .map_err(|e: String| anyhow::anyhow!("{}", e))?;

    match engine_type {
        EngineType::DuckDuckGo => Ok(Box::new(engine::duckduckgo::DuckDuckGo::new(None)?)),
        EngineType::Google => Ok(Box::new(engine::google::Google::new(None)?)),
        EngineType::Bing => Ok(Box::new(engine::bing::Bing::new(None)?)),
        EngineType::Brave => Ok(Box::new(engine::brave::Brave::new(None)?)),
        EngineType::SearXNG => {
            let base_url = cli
                .searxng_url
                .clone()
                .unwrap_or_else(|| "http://localhost:8080".to_string());
            Ok(Box::new(engine::searxng::SearXNG::new(&base_url, None)?))
        }
        EngineType::Auto => {
            let base_url = cli
                .searxng_url
                .clone()
                .unwrap_or_else(|| "http://localhost:8080".to_string());
            Ok(Box::new(engine::fusion::AutoEngine::new(&base_url, None)?))
        }
    }
}

/// 打印搜索结果
fn print_results(
    results: &[engine::SearchResult],
    engine: &str,
    query: &str,
    elapsed: std::time::Duration,
) {
    if results.is_empty() {
        println!("  (no results)");
        return;
    }

    println!(
        "  {} results for '{}' ({} engine, took {} ms)\n",
        results.len(),
        query,
        engine,
        elapsed.as_millis()
    );

    for (i, r) in results.iter().enumerate() {
        println!("  {}. \x1b[1m{}\x1b[0m", i + 1, r.title);
        println!("     \x1b[2m{}\x1b[0m", r.url);
        if !r.snippet.is_empty() {
            println!("     {}", r.snippet);
        }
        if let Some(ref content) = r.content {
            if !content.is_empty() {
                let preview: String = content.chars().take(120).collect();
                println!("     \x1b[3m> {}\x1b[0m", preview);
            }
        }
        println!();
    }
}

/// 在浏览器中打开 URL
fn open_in_browser(url: &str) {
    // 检测操作系统
    let result = if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).status()
    } else if cfg!(target_os = "linux") {
        // 尝试 xdg-open，失败则尝试其他
        std::process::Command::new("xdg-open")
            .arg(url)
            .status()
            .or_else(|_| {
                std::process::Command::new("x-www-browser")
                    .arg(url)
                    .status()
            })
            .or_else(|_| std::process::Command::new("w3m").arg(url).status())
    } else if cfg!(target_os = "windows") {
        std::process::Command::new("cmd")
            .args(["/c", "start", url])
            .status()
    } else {
        #[allow(clippy::io_other_error)]
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "unsupported OS",
        ))
    };

    match result {
        Ok(_) => println!("  \x1b[32m✓\x1b[0m Opened in browser"),
        Err(e) => eprintln!("  \x1b[31m✗\x1b[0m Failed to open browser: {}", e),
    }
}

/// 显示帮助信息
fn print_help() {
    println!();
    println!("  \x1b[1mCommands:\x1b[0m");
    println!("  <query>           Search the web");
    println!("  n, next           Next page");
    println!("  p, prev           Previous page");
    println!("  o <N>, open <N>   Open result N in browser");
    println!("  f <N>, fetch <N>  Fetch full content of result N");
    println!("  h, help, ?        Show this help");
    println!("  q, quit, exit     Exit REPL");
    println!();
    println!("  \x1b[1mTips:\x1b[0m");
    println!("  - Use ↑/↓ for history navigation");
    println!("  - Results are cached in memory for the session");
    println!();
}

/// 获取单个结果的完整内容
async fn fetch_result_content(state: &ReplState, index: usize) -> anyhow::Result<()> {
    if index == 0 || index > state.results.len() {
        anyhow::bail!(
            "Invalid result number. Valid range: 1-{}",
            state.results.len()
        );
    }

    let result = &state.results[index - 1];
    println!("  Fetching: {}", result.url);
    println!();

    let fetcher_config = fetcher::FetcherConfig {
        max_content_length: 5000,
        ..Default::default()
    };
    let f = fetcher::Fetcher::new(fetcher_config)?;
    let mut results = vec![result.clone()];
    f.fetch(&mut results).await;

    if let Some(ref content) = results[0].content {
        println!("  \x1b[1mContent:\x1b[0m");
        for line in content.lines() {
            println!("  {}", line);
        }
    } else {
        println!("  \x1b[31mFailed to fetch content or empty page.\x1b[0m");
    }

    Ok(())
}

/// 处理翻页命令
fn handle_page_cmd(
    state: &ReplState,
    direction: PageDirection,
) -> anyhow::Result<Option<(String, u32)>> {
    match direction {
        PageDirection::Next => {
            if state.last_query.is_empty() {
                println!("  No previous search. Enter a query first.");
                return Ok(None);
            }
            Ok(Some((state.last_query.clone(), state.page + 1)))
        }
        PageDirection::Prev => {
            if state.page <= 1 {
                println!("  Already on the first page.");
                return Ok(None);
            }
            Ok(Some((state.last_query.clone(), state.page - 1)))
        }
    }
}

enum PageDirection {
    Next,
    Prev,
}

/// 验证结果序号是否有效
fn validate_index(index: usize, total: usize) -> bool {
    if index == 0 || index > total {
        if total > 0 {
            println!("  Invalid number. Valid range: 1-{}", total);
        }
        return false;
    }
    true
}

/// 处理单个 REPL 命令
async fn handle_command(cmd: ReplCommand, state: &mut ReplState) -> anyhow::Result<CommandResult> {
    match cmd {
        ReplCommand::Quit => return Ok(CommandResult::Quit),
        ReplCommand::Help => {
            print_help();
        }
        other => {
            handle_action(other, state).await?;
        }
    }
    Ok(CommandResult::Continue)
}

/// 处理搜索/翻页/打开/获取等操作命令
async fn handle_action(cmd: ReplCommand, state: &mut ReplState) -> anyhow::Result<()> {
    match cmd {
        ReplCommand::Search(query) if !query.is_empty() => {
            do_search(state, &query, 1).await?;
        }
        ReplCommand::NextPage => {
            go_to_page(state, PageDirection::Next).await?;
        }
        ReplCommand::PrevPage => {
            go_to_page(state, PageDirection::Prev).await?;
        }
        ReplCommand::Open(index) if validate_index(index, state.results.len()) => {
            open_in_browser(&state.results[index - 1].url);
        }
        ReplCommand::Fetch(index) if validate_index(index, state.results.len()) => {
            fetch_result_content(state, index).await?;
        }
        _ => {}
    }
    Ok(())
}

/// 翻页到下一页/上一页
async fn go_to_page(state: &mut ReplState, direction: PageDirection) -> anyhow::Result<()> {
    if let Some((q, p)) = handle_page_cmd(state, direction)? {
        do_search(state, &q, p).await?;
    }
    Ok(())
}

/// REPL 命令执行结果
enum CommandResult {
    Continue,
    Quit,
}

/// 启动 REPL 主循环
pub async fn run_repl(cli: Cli) -> anyhow::Result<()> {
    let config = Config::builder()
        .edit_mode(EditMode::Emacs)
        .history_ignore_space(true)
        .build();

    let mut rl = Editor::with_config(config)?;
    rl.set_helper(Some(ReplHelper));
    // 尝试加载历史记录
    let _ = rl.load_history(
        &dirs::cache_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("seekit")
            .join("repl_history.txt"),
    );

    let mut state = ReplState::new(cli);

    println!();
    println!("  \x1b[1mseekit REPL\x1b[0m — Type a query to search, or 'h' for help.");
    println!();

    loop {
        let prompt = "\x1b[1m» \x1b[0m";
        let line = match rl.readline(prompt) {
            Ok(line) => line,
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => {
                println!();
                break;
            }
            Err(e) => {
                eprintln!("REPL error: {}", e);
                break;
            }
        };

        let trimmed = line.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }
        rl.add_history_entry(&trimmed)?;

        match handle_command(parse_command(&trimmed), &mut state).await {
            Ok(CommandResult::Quit) => {
                println!("  Goodbye!");
                break;
            }
            Ok(CommandResult::Continue) => {}
            Err(e) => eprintln!("  \x1b[31mError:\x1b[0m {}", e),
        }
    }

    // 保存历史记录
    if let Some(cache_dir) = dirs::cache_dir() {
        let seekit_dir = cache_dir.join("seekit");
        let _ = std::fs::create_dir_all(&seekit_dir);
        let _ = rl.save_history(&seekit_dir.join("repl_history.txt"));
    }

    Ok(())
}
