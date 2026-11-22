//! Authoritative, deterministic game rules for the fear card game.
//!
//! This crate has NO dependency on the engine, a window, or a GPU. It can be
//! played in a terminal (`cargo run -p game-rules --bin play`), unit-tested,
//! saved and replayed.
//!
//! Rules of thumb that keep it deterministic:
//! - integer math only, no floats, no wall-clock time
//! - only `Vec` and `BTreeMap` (never `HashMap`)
//! - randomness only from the seeded [`Pcg32State`] stored inside the state
//! - the ONLY way to change a [`GameState`] is [`GameState::apply`]
//!
//! ## The mechanic (the "Anxiety Clock"; deliberately small and throwaway)
//! - Playing a card changes anxiety by `exposure - calm` and adds the card's
//!   `power` to `progress`.
//! - Ending the turn discards your hand, raises anxiety by `turn_anxiety`,
//!   and draws a new hand.
//! - Anxiety reaching `max_anxiety` means you panic and LOSE.
//!   Progress reaching `win_target` means you WIN.
//!   If both happen on the same play, panic wins (you lose).

mod rng;

pub use rng::{Pcg32State, DEFAULT_STREAM};

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Default card definitions, shipped with the crate (`data/cards.ron`).
pub const DEFAULT_CARDS_RON: &str = include_str!("../data/cards.ron");
/// Default tuning numbers, shipped with the crate (`data/rules.ron`).
pub const DEFAULT_RULES_RON: &str = include_str!("../data/rules.ron");

/// The type of fear. Dictates flavor and future interactions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum FearKind {
    /// Medically recognised phobia (e.g. thalassophobia).
    MedicalPhobia,
    /// Highly specific, situational fear (e.g. a snake in a toilet).
    NicheFear,
}

/// The immutable definition of a card, loaded from RON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardDef {
    /// Stable id. Piles store only this, never a copy of the card.
    pub id: u32,
    /// Display name.
    pub name: String,
    /// Kind of fear.
    pub kind: FearKind,
    /// Anxiety added when played.
    pub exposure: i32,
    /// Anxiety removed when played.
    pub calm: i32,
    /// Progress added when played.
    pub power: i32,
}

/// The global tuning rules, loaded from RON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rules {
    /// Cards drawn at the start of each turn.
    pub hand_size: u8,
    /// Anxiety added when a turn ends.
    pub turn_anxiety: i32,
    /// Anxiety at which you panic (lose).
    pub max_anxiety: i32,
    /// Progress required to win.
    pub win_target: i32,
}

/// Current game status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    /// Still being played.
    Playing,
    /// Reached the win target.
    Won,
    /// Anxiety hit the maximum.
    Lost,
}

/// The actions a player can take. The only way to mutate [`GameState`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    /// Play the card at this position in the hand (0 = first).
    PlayCard {
        /// Index into the hand.
        hand_index: u32,
    },
    /// End the turn.
    EndTurn,
}

/// Why an action was refused. A refused action never changes the state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameError {
    /// The game is already won or lost.
    GameOver,
    /// There is no card at that hand position.
    InvalidHandIndex {
        /// The index that was asked for.
        index: u32,
        /// How many cards are in the hand.
        hand_len: u32,
    },
    /// A pile holds an id that has no definition.
    UnknownCard(u32),
}

impl std::fmt::Display for GameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameError::GameOver => write!(f, "the game is over"),
            GameError::InvalidHandIndex { index, hand_len } => {
                write!(f, "no card at hand index {index} (hand has {hand_len} cards)")
            }
            GameError::UnknownCard(id) => write!(f, "card id {id} has no definition"),
        }
    }
}

impl std::error::Error for GameError {}

/// Why card or rules data could not be loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataError {
    /// The RON text could not be parsed.
    Parse(String),
    /// Two cards share the same id.
    DuplicateId(u32),
    /// The card list is empty.
    NoCards,
}

impl std::fmt::Display for DataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataError::Parse(msg) => write!(f, "could not parse data: {msg}"),
            DataError::DuplicateId(id) => write!(f, "duplicate card id {id}"),
            DataError::NoCards => write!(f, "the card list is empty"),
        }
    }
}

impl std::error::Error for DataError {}

/// Parse a RON list of cards into a lookup table keyed by id.
pub fn cards_from_ron(text: &str) -> Result<BTreeMap<u32, CardDef>, DataError> {
    let list: Vec<CardDef> = ron::from_str(text).map_err(|e| DataError::Parse(e.to_string()))?;
    let mut map = BTreeMap::new();
    for card in list {
        let id = card.id;
        if map.insert(id, card).is_some() {
            return Err(DataError::DuplicateId(id));
        }
    }
    if map.is_empty() {
        return Err(DataError::NoCards);
    }
    Ok(map)
}

/// Parse RON tuning numbers.
pub fn rules_from_ron(text: &str) -> Result<Rules, DataError> {
    ron::from_str(text).map_err(|e| DataError::Parse(e.to_string()))
}

/// The authoritative game state.
///
/// Piles hold only card IDs. The real [`CardDef`]s live in a `BTreeMap`
/// passed to [`GameState::apply`], so changing a card's stats in RON (hot
/// reload, later) instantly affects cards already in the deck or hand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameState {
    /// Tuning numbers for this game.
    pub rules: Rules,
    /// Playing, Won or Lost.
    pub outcome: Outcome,
    /// Current anxiety (0 to `max_anxiety`).
    pub anxiety: i32,
    /// Progress toward `win_target`.
    pub progress: i32,
    /// Current turn number, starting at 1.
    pub turn: u32,
    /// Seed this game started with.
    pub seed: u64,
    /// Full random generator state (saved so loading continues identically).
    pub rng: Pcg32State,
    /// Shuffled draw pile. The top card is the LAST element.
    pub deck: Vec<u32>,
    /// Cards you can play right now.
    pub hand: Vec<u32>,
    /// Played or discarded cards.
    pub discard: Vec<u32>,
}

impl GameState {
    /// Start a new game: one copy of every defined card, shuffled, then draw
    /// the first hand.
    pub fn new(rules: Rules, defs: &BTreeMap<u32, CardDef>, seed: u64) -> Self {
        let mut rng = Pcg32State::new(seed);
        let mut deck: Vec<u32> = defs.keys().copied().collect();
        rng.shuffle(&mut deck);
        let mut state = Self {
            rules,
            outcome: Outcome::Playing,
            anxiety: 0,
            progress: 0,
            turn: 1,
            seed,
            rng,
            deck,
            hand: Vec::new(),
            discard: Vec::new(),
        };
        state.draw_hand();
        state
    }

    /// The only way to change the state. A refused action leaves the state
    /// completely untouched.
    pub fn apply(
        &mut self,
        action: Action,
        defs: &BTreeMap<u32, CardDef>,
    ) -> Result<(), GameError> {
        if self.outcome != Outcome::Playing {
            return Err(GameError::GameOver);
        }
        match action {
            Action::PlayCard { hand_index } => {
                let idx = hand_index as usize;
                let id = *self.hand.get(idx).ok_or(GameError::InvalidHandIndex {
                    index: hand_index,
                    hand_len: self.hand.len() as u32,
                })?;
                let def = defs.get(&id).ok_or(GameError::UnknownCard(id))?;

                // All checks passed: now it is safe to change things.
                self.hand.remove(idx);
                self.discard.push(id);
                let net = def.exposure.saturating_sub(def.calm);
                self.anxiety = self.clamp_anxiety(self.anxiety.saturating_add(net));
                self.progress = self.progress.saturating_add(def.power);
                self.update_outcome();
            }
            Action::EndTurn => {
                self.discard.append(&mut self.hand);
                let raised = self.anxiety.saturating_add(self.rules.turn_anxiety);
                self.anxiety = self.clamp_anxiety(raised);
                self.update_outcome();
                if self.outcome == Outcome::Playing {
                    self.turn += 1;
                    self.draw_hand();
                }
            }
        }
        Ok(())
    }

    /// A fingerprint of the whole state: FNV-1a over the integer fields in a
    /// fixed order. Equal hashes mean equal states (for practical purposes).
    ///
    /// Order: outcome, rules, anxiety, progress, turn, seed, rng.state,
    /// rng.inc, then each pile (length first, then its ids).
    pub fn hash(&self) -> u64 {
        let mut h = Fnv::new();
        h.u8(match self.outcome {
            Outcome::Playing => 0,
            Outcome::Won => 1,
            Outcome::Lost => 2,
        });
        h.u8(self.rules.hand_size);
        h.i32(self.rules.turn_anxiety);
        h.i32(self.rules.max_anxiety);
        h.i32(self.rules.win_target);
        h.i32(self.anxiety);
        h.i32(self.progress);
        h.u32(self.turn);
        h.u64(self.seed);
        h.u64(self.rng.state);
        h.u64(self.rng.inc);
        for pile in [&self.deck, &self.hand, &self.discard] {
            h.u32(pile.len() as u32);
            for id in pile {
                h.u32(*id);
            }
        }
        h.0
    }

    fn clamp_anxiety(&self, value: i32) -> i32 {
        value.max(0).min(self.rules.max_anxiety.max(0))
    }

    fn update_outcome(&mut self) {
        if self.anxiety >= self.rules.max_anxiety {
            self.outcome = Outcome::Lost;
        } else if self.progress >= self.rules.win_target {
            self.outcome = Outcome::Won;
        }
    }

    fn draw_hand(&mut self) {
        for _ in 0..self.rules.hand_size {
            if !self.draw_one() {
                break;
            }
        }
    }

    /// Draw one card. When the deck is empty, the discard pile is shuffled
    /// back in. Returns false if there is nothing left to draw.
    fn draw_one(&mut self) -> bool {
        if self.deck.is_empty() {
            if self.discard.is_empty() {
                return false;
            }
            self.deck.append(&mut self.discard);
            self.rng.shuffle(&mut self.deck);
        }
        match self.deck.pop() {
            Some(id) => {
                self.hand.push(id);
                true
            }
            None => false,
        }
    }
}

/// A recorded game: the seed plus every action, in order. Replaying it must
/// always give the same final state hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Replay {
    /// Seed the game started with.
    pub seed: u64,
    /// Every accepted action, in order.
    pub actions: Vec<Action>,
}

impl Replay {
    /// Play the whole recording from the start and return the final state.
    pub fn run(
        &self,
        rules: Rules,
        defs: &BTreeMap<u32, CardDef>,
    ) -> Result<GameState, GameError> {
        let mut state = GameState::new(rules, defs, self.seed);
        for action in &self.actions {
            state.apply(action.clone(), defs)?;
        }
        Ok(state)
    }
}

/// A simple bot used for balance testing. Among the cards it can afford
/// without risking panic (it keeps room for the end-of-turn anxiety), it plays
/// the one with the best power per point of anxiety. If none is safe, it ends
/// the turn. A card that wins the game on the spot is always played.
pub fn greedy_action(state: &GameState, defs: &BTreeMap<u32, CardDef>) -> Action {
    let mut best: Option<(u32, i64)> = None; // (hand index, score)
    for (i, id) in state.hand.iter().enumerate() {
        let Some(def) = defs.get(id) else { continue };
        let net = def.exposure.saturating_sub(def.calm);
        let after = state.anxiety.saturating_add(net);
        let wins = state.progress.saturating_add(def.power) >= state.rules.win_target
            && after < state.rules.max_anxiety;
        let safe = after.saturating_add(state.rules.turn_anxiety) < state.rules.max_anxiety;
        if !wins && !safe {
            continue;
        }
        let score = if wins {
            i64::MAX
        } else {
            (def.power as i64) * 100 / (net.max(1) as i64)
        };
        if best.map_or(true, |(_, s)| score > s) {
            best = Some((i as u32, score));
        }
    }
    match best {
        Some((hand_index, _)) => Action::PlayCard { hand_index },
        None => Action::EndTurn,
    }
}

/// FNV-1a, 64-bit, written by hand so the hash never depends on a library
/// version. Integers are fed in little-endian byte order.
struct Fnv(u64);

impl Fnv {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    fn new() -> Self {
        Self(Self::OFFSET)
    }
    fn bytes(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 ^= *b as u64;
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
    }
    fn u8(&mut self, v: u8) {
        self.bytes(&[v]);
    }
    fn u32(&mut self, v: u32) {
        self.bytes(&v.to_le_bytes());
    }
    fn i32(&mut self, v: i32) {
        self.bytes(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.bytes(&v.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (Rules, BTreeMap<u32, CardDef>) {
        (
            rules_from_ron(DEFAULT_RULES_RON).expect("rules.ron must parse"),
            cards_from_ron(DEFAULT_CARDS_RON).expect("cards.ron must parse"),
        )
    }

    /// Play a whole game with the bot, returning the final state.
    fn bot_game(seed: u64, rules: Rules, defs: &BTreeMap<u32, CardDef>) -> (GameState, Vec<Action>) {
        let mut state = GameState::new(rules, defs, seed);
        let mut actions = Vec::new();
        for _ in 0..10_000 {
            if state.outcome != Outcome::Playing {
                break;
            }
            let a = greedy_action(&state, defs);
            state.apply(a.clone(), defs).expect("bot actions are legal");
            actions.push(a);
        }
        (state, actions)
    }

    #[test]
    fn shipped_data_parses() {
        let (rules, defs) = setup();
        assert_eq!(defs.len(), 10);
        assert!(rules.max_anxiety > 0);
    }

    #[test]
    fn duplicate_ids_and_empty_lists_are_rejected() {
        let dup = r#"[(id: 1, name: "a", kind: NicheFear, exposure: 1, calm: 0, power: 1),
                      (id: 1, name: "b", kind: NicheFear, exposure: 1, calm: 0, power: 1)]"#;
        assert_eq!(cards_from_ron(dup), Err(DataError::DuplicateId(1)));
        assert_eq!(cards_from_ron("[]"), Err(DataError::NoCards));
        assert!(matches!(cards_from_ron("nonsense"), Err(DataError::Parse(_))));
    }

    #[test]
    fn new_game_deals_a_hand() {
        let (rules, defs) = setup();
        let s = GameState::new(rules, &defs, 1);
        assert_eq!(s.hand.len(), rules.hand_size as usize);
        assert_eq!(s.hand.len() + s.deck.len() + s.discard.len(), defs.len());
        assert_eq!((s.outcome, s.anxiety, s.progress, s.turn), (Outcome::Playing, 0, 0, 1));
    }

    #[test]
    fn same_seed_and_actions_give_same_hash() {
        let (rules, defs) = setup();
        let (a, actions) = bot_game(7, rules, &defs);
        let replay = Replay { seed: 7, actions };
        let b = replay.run(rules, &defs).expect("replay is legal");
        assert_eq!(a.hash(), b.hash());
        assert_eq!(a, b);
    }

    #[test]
    fn different_seeds_usually_differ() {
        let (rules, defs) = setup();
        let a = GameState::new(rules, &defs, 1);
        let b = GameState::new(rules, &defs, 2);
        assert_ne!(a.hash(), b.hash());
    }

    #[test]
    fn save_mid_game_and_continue_matches_never_saving() {
        let (rules, defs) = setup();
        // Game A: play 12 bot actions straight through.
        let mut a = GameState::new(rules, &defs, 5);
        let mut mid = None;
        for step in 0..12 {
            if a.outcome != Outcome::Playing {
                break;
            }
            if step == 6 {
                mid = Some(ron::to_string(&a).expect("serialize"));
            }
            let act = greedy_action(&a, &defs);
            a.apply(act, &defs).expect("legal");
        }
        // Game B: load the mid-game save and play the same remaining steps.
        let mut b: GameState = ron::from_str(&mid.expect("saved")).expect("load");
        for _ in 6..12 {
            if b.outcome != Outcome::Playing {
                break;
            }
            let act = greedy_action(&b, &defs);
            b.apply(act, &defs).expect("legal");
        }
        assert_eq!(a.hash(), b.hash());
        assert_eq!(a, b);
    }

    #[test]
    fn state_round_trips_through_ron() {
        let (rules, defs) = setup();
        let (state, _) = bot_game(3, rules, &defs);
        let text = ron::to_string(&state).expect("serialize");
        let back: GameState = ron::from_str(&text).expect("deserialize");
        assert_eq!(state, back);
    }

    #[test]
    fn refused_actions_do_not_change_state() {
        let (rules, defs) = setup();
        let mut s = GameState::new(rules, &defs, 1);
        let before = s.clone();
        let err = s.apply(Action::PlayCard { hand_index: 99 }, &defs);
        assert_eq!(err, Err(GameError::InvalidHandIndex { index: 99, hand_len: rules.hand_size as u32 }));
        assert_eq!(s, before);
    }

    #[test]
    fn unknown_card_id_is_an_error_not_a_panic() {
        let (rules, mut defs) = setup();
        let mut s = GameState::new(rules, &defs, 1);
        let id = s.hand[0];
        defs.remove(&id);
        let before = s.clone();
        assert_eq!(s.apply(Action::PlayCard { hand_index: 0 }, &defs), Err(GameError::UnknownCard(id)));
        assert_eq!(s, before);
    }

    #[test]
    fn game_over_rejects_further_actions() {
        let (rules, defs) = setup();
        let (mut s, _) = bot_game(1, rules, &defs);
        assert_ne!(s.outcome, Outcome::Playing);
        assert_eq!(s.apply(Action::EndTurn, &defs), Err(GameError::GameOver));
    }

    #[test]
    fn anxiety_is_clamped_at_zero_and_panic_loses() {
        let (rules, defs) = setup();
        // Card 5 (Coulrophobia) has net -6: at anxiety 0 it must stay at 0.
        let mut s = GameState::new(rules, &defs, 1);
        s.hand = vec![5];
        s.apply(Action::PlayCard { hand_index: 0 }, &defs).expect("legal");
        assert_eq!(s.anxiety, 0);
        // Push anxiety to the edge, then play a scary card: panic.
        let mut s = GameState::new(rules, &defs, 1);
        s.anxiety = 90;
        s.hand = vec![6];
        s.apply(Action::PlayCard { hand_index: 0 }, &defs).expect("legal");
        assert_eq!((s.anxiety, s.outcome), (100, Outcome::Lost));
    }

    #[test]
    fn reaching_win_target_wins() {
        let (rules, defs) = setup();
        let mut s = GameState::new(rules, &defs, 1);
        s.progress = rules.win_target - 1;
        s.hand = vec![8];
        s.apply(Action::PlayCard { hand_index: 0 }, &defs).expect("legal");
        assert_eq!(s.outcome, Outcome::Won);
    }

    #[test]
    fn editing_a_card_def_affects_cards_already_in_hand() {
        let (rules, mut defs) = setup();
        let mut s = GameState::new(rules, &defs, 1);
        s.hand = vec![1];
        defs.get_mut(&1).expect("card 1").exposure = 3; // "hot reload"
        s.apply(Action::PlayCard { hand_index: 0 }, &defs).expect("legal");
        assert_eq!(s.anxiety, 3);
    }

    #[test]
    fn discard_is_reshuffled_when_deck_runs_out() {
        let (rules, defs) = setup();
        let mut s = GameState::new(rules, &defs, 1);
        for _ in 0..6 {
            if s.outcome != Outcome::Playing {
                break;
            }
            s.apply(Action::EndTurn, &defs).expect("legal");
            if s.outcome == Outcome::Playing {
                assert_eq!(s.hand.len() + s.deck.len() + s.discard.len(), defs.len());
                assert_eq!(s.hand.len(), rules.hand_size as usize);
            }
        }
    }
}
