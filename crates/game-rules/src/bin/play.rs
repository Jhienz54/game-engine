//! Play the fear card game in the terminal. No window, no engine.
//!
//!   cargo run -p game-rules --bin play                 (play, seed 1)
//!   cargo run -p game-rules --bin play -- --seed 42    (play another seed)
//!   cargo run -p game-rules --bin play -- --replay last_game.ron
//!   cargo run -p game-rules --bin play -- --sim 1000   (bot plays 1000 games)
//!
//! While playing: type a card number to play it, `e` to end the turn,
//! `h` to show the state hash, `q` to quit. Every game is saved to
//! `last_game.ron`, which `--replay` can run again.

use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};

use game_rules::{
    cards_from_ron, greedy_action, rules_from_ron, Action, CardDef, GameState, Outcome, Replay,
    Rules, DEFAULT_CARDS_RON, DEFAULT_RULES_RON,
};

type AnyError = Box<dyn std::error::Error>;

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), AnyError> {
    let defs = cards_from_ron(DEFAULT_CARDS_RON)?;
    let rules = rules_from_ron(DEFAULT_RULES_RON)?;

    let mut seed: u64 = 1;
    let mut replay_path: Option<String> = None;
    let mut sim_games: Option<u64> = None;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--seed" => seed = it.next().ok_or("--seed needs a number")?.parse()?,
            "--replay" => replay_path = Some(it.next().ok_or("--replay needs a file")?.clone()),
            "--sim" => sim_games = Some(it.next().ok_or("--sim needs a number")?.parse()?),
            "--help" | "-h" => {
                println!("options: --seed N | --replay FILE | --sim N");
                return Ok(());
            }
            other => return Err(format!("unknown argument: {other} (try --help)").into()),
        }
    }

    if let Some(path) = replay_path {
        return replay_file(&path, rules, &defs);
    }
    if let Some(n) = sim_games {
        simulate(n, rules, &defs);
        return Ok(());
    }
    play_interactive(seed, rules, &defs)
}

fn replay_file(path: &str, rules: Rules, defs: &BTreeMap<u32, CardDef>) -> Result<(), AnyError> {
    let text = std::fs::read_to_string(path)?;
    let replay: Replay = ron::from_str(&text)?;
    let state = replay.run(rules, defs)?;
    println!("replayed {} actions from seed {}", replay.actions.len(), replay.seed);
    print_state(&state, defs);
    println!("final hash: {:016x}", state.hash());
    Ok(())
}

fn simulate(n: u64, rules: Rules, defs: &BTreeMap<u32, CardDef>) {
    let (mut won, mut lost, mut unfinished, mut turns_total) = (0u64, 0u64, 0u64, 0u64);
    for seed in 0..n {
        let mut state = GameState::new(rules, defs, seed);
        for _ in 0..10_000 {
            if state.outcome != Outcome::Playing {
                break;
            }
            let action = greedy_action(&state, defs);
            if state.apply(action, defs).is_err() {
                break;
            }
        }
        match state.outcome {
            Outcome::Won => won += 1,
            Outcome::Lost => lost += 1,
            Outcome::Playing => unfinished += 1,
        }
        turns_total += state.turn as u64;
    }
    let pct = |x: u64| if n == 0 { 0.0 } else { 100.0 * x as f64 / n as f64 };
    println!("greedy bot, {n} games:");
    println!("  won:        {won} ({:.1}%)", pct(won));
    println!("  lost:       {lost} ({:.1}%)", pct(lost));
    println!("  unfinished: {unfinished}");
    println!("  avg turns:  {:.1}", if n == 0 { 0.0 } else { turns_total as f64 / n as f64 });
}

fn play_interactive(seed: u64, rules: Rules, defs: &BTreeMap<u32, CardDef>) -> Result<(), AnyError> {
    let mut state = GameState::new(rules, defs, seed);
    let mut actions: Vec<Action> = Vec::new();
    let stdin = io::stdin();

    println!("Fear cards (seed {seed}). Type a card number, e = end turn, h = hash, q = quit.");
    loop {
        print_state(&state, defs);
        if state.outcome != Outcome::Playing {
            break;
        }
        print!("> ");
        io::stdout().flush()?;
        let mut line = String::new();
        if stdin.lock().read_line(&mut line)? == 0 {
            break; // end of input
        }
        let action = match line.trim() {
            "q" => break,
            "e" => Action::EndTurn,
            "h" => {
                println!("hash: {:016x}", state.hash());
                continue;
            }
            other => match other.parse::<u32>() {
                Ok(i) => Action::PlayCard { hand_index: i },
                Err(_) => {
                    println!("?  (card number, e, h or q)");
                    continue;
                }
            },
        };
        match state.apply(action.clone(), defs) {
            Ok(()) => actions.push(action),
            Err(e) => println!("not allowed: {e}"),
        }
    }

    let replay = Replay { seed, actions };
    let text = ron::ser::to_string_pretty(&replay, ron::ser::PrettyConfig::default())?;
    std::fs::write("last_game.ron", text)?;
    println!("final hash: {:016x}   (saved last_game.ron)", state.hash());
    Ok(())
}

fn print_state(state: &GameState, defs: &BTreeMap<u32, CardDef>) {
    println!();
    println!(
        "Turn {}   Anxiety {}/{}   Progress {}/{}   Deck {}  Discard {}",
        state.turn,
        state.anxiety,
        state.rules.max_anxiety,
        state.progress,
        state.rules.win_target,
        state.deck.len(),
        state.discard.len(),
    );
    for (i, id) in state.hand.iter().enumerate() {
        match defs.get(id) {
            Some(c) => println!(
                "  [{i}] {:<34} exposure +{:<2} calm -{:<2} power {}",
                c.name, c.exposure, c.calm, c.power
            ),
            None => println!("  [{i}] (unknown card {id})"),
        }
    }
    match state.outcome {
        Outcome::Won => println!("*** YOU WIN ***"),
        Outcome::Lost => println!("*** PANIC! You lose. ***"),
        Outcome::Playing => {}
    }
}
