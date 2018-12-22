
// Value is the value the consensus seeks agreement on.
#[derive(Copy, Clone, PartialEq)]
struct Value{}

// RoundValue contains a Value and the round it was set.
#[derive(Copy, Clone, PartialEq)]
struct RoundValue{
    round: i64,
    value: Value
}

// State is the state of the consensus.
#[derive(Copy, Clone)]
struct State{
    height: i64,
    round: i64,
    step: RoundStep,
    locked: Option<RoundValue>,
    valid: Option<RoundValue>,
}

impl State{
    fn set_round(self, round: i64) -> State{
        State{
            round: round,
            ..self
        }
    }

    fn set_step(self, step: RoundStep) -> State{
        State{
            step: step,
            ..self
        }
    }

    fn set_locked(self, locked: Value) -> State{
        State{
            locked: Some(RoundValue{round: self.round, value: locked}),
            ..self
        }
    }

    fn set_valid(self, valid: Value) -> State{
        State{
            valid: Some(RoundValue{round: self.round, value: valid}),
            ..self
        }
    }
}

// RoundStep is the step of the consensus in the round.
#[derive(Copy, Clone)]
enum RoundStep {
    NewRound,
    Propose,
    Prevote,
    Precommit,
    Commit,
}

// Event causes a state transition.
enum Event {
    NewRound(i64),
    NewRoundProposer(i64, Value),
    Proposal(i64, Value),
    ProposalInvalid(i64),
    ProposalPolka(i64, i64, Value),
    PolkaAny(i64),
    PolkaNil(i64),
    PolkaValue(i64, Value),
    PrecommitAny(i64),
    PrecommitValue(i64, Value),
    RoundSkip(i64),
    RoundSkipProposer(i64, Value),
    TimeoutPropose(i64),
    TimeoutPrevote(i64),
    TimeoutPrecommit(i64),
    TimeoutPrecommitProposer(i64, Value),
}

// Message is returned.
enum Message {
    NewRound,
    Proposal(Proposal),
    Prevote(Vote),
    Precommit(Vote),
    Timeout(Timeout),
    Decision(RoundValue),
}

struct Proposal{
    round: i64,
    value: Value,
    pol_round: i64,
}

impl Proposal{
    fn new(round: i64, value: Value, pol_round: i64) -> Proposal{
        Proposal{
            round: round,
            value: value,
            pol_round: pol_round,
        }
    }
}

struct Vote{
    round: i64,
    value: Option<Value>,
}

impl Vote {
    fn new(round: i64, value: Option<Value>) -> Vote{
        Vote{
            round: round,
            value: value,
        }
    }
}

struct Timeout{
    round: i64,
    step: RoundStep,
}

impl Timeout{
    fn new(round: i64, step: RoundStep) -> Timeout{
        Timeout{
            round: round,
            step: step,
        }
    }

}

impl State{
    fn new(height: i64) -> State{
        State{
            height: height,
            round: 0,
            step: RoundStep::NewRound,
            locked: None,
            valid: None,
        }
    }

    fn next(self, event: Event) -> (State, Option<Message>) {
        let (s, round) = (self, self.round);
        let (s, m) = match (s.step, event) {
            (RoundStep::NewRound, Event::NewRoundProposer(r, v)) => { handle_new_round_proposer(s, r, v) } // 11/14
            (RoundStep::NewRound, Event::NewRound(r)) => { handle_new_round(s, r) } // 11/20
            (RoundStep::Propose, Event::Proposal(r, v)) if round == r => { handle_proposal(s, v) } // 22
            (RoundStep::Propose, Event::ProposalInvalid(r)) if round == r => { handle_proposal_invalid(s) } // 22/25, 28/31
            (RoundStep::Propose, Event::ProposalPolka(r, vr, v)) if round == r => { handle_proposal_polka(s, vr, v) } // 28
            (RoundStep::Propose, Event::TimeoutPropose(r)) if round == r => { handle_timeout_propose(s) } // 57
            (RoundStep::Prevote, Event::PolkaAny(r)) if round == r => { handle_polka_any(s) } // 34
            (RoundStep::Prevote, Event::PolkaNil(r)) if round == r => { handle_polka_nil(s) } // 44
            (RoundStep::Prevote, Event::PolkaValue(r, v)) if round == r => { handle_polka_value_prevote(s, v) } // 36/37 - only once?
            (RoundStep::Prevote, Event::TimeoutPrevote(r)) if round == r => { handle_timeout_prevote(s) } // 61
            (RoundStep::Precommit, Event::PolkaValue(r, v)) if round == r => { handle_polka_value_precommit(s, v) } // 36/42 - only once?
            (_,                    Event::PrecommitAny(r)) if round == r => { handle_precommit_any(s) } // 47
            (_,                    Event::PrecommitValue(r, v)) => { handle_precommit_value(s, r, v) } // 49
            (_,                    Event::RoundSkipProposer(r, v)) if round < r => { handle_new_round_proposer(s, r, v) } // 55
            (_,                    Event::RoundSkip(r)) if round < r => { handle_new_round(s, r) } // 55
            (_,                    Event::TimeoutPrecommitProposer(r, v)) if round == r=> { handle_new_round_proposer(s, r+1, v) } // 65
            (_,                    Event::TimeoutPrecommit(r)) => { handle_new_round(s, r+1) } // 65
            _ => { (s, None) }
        };
        (s, m)
    }
}

// we're the proposer. decide a propsal.
// 11/14
fn handle_new_round_proposer(s: State, r: i64, v: Value) -> (State, Option<Message>) {
    let s = s.set_round(r).set_step(RoundStep::Propose);
    let (value, round) = match s.valid {
        Some(v) => { (v.value, v.round) }
        None    => { (v, -1) } 
    };
    (s, Some(Message::Proposal(Proposal::new(r, value, round))))
}


// we're not the proposer. schedule timeout propose
// 11/20
fn handle_new_round(s: State, r: i64) -> (State, Option<Message>) {
    let s = s.set_round(r).set_step(RoundStep::Propose);
    (s, Some(Message::Timeout(Timeout::new(s.round, s.step))))
}

// received a complete proposal with new value - prevote
// 22
fn handle_proposal(s: State, proposed: Value) -> (State, Option<Message>){
    let s = s.set_step(RoundStep::Prevote);
    let value = match s.locked {
        Some(locked) if proposed != locked.value => { None } // locked on something else
        _ => { Some(proposed) } 
    };
    (s, Some(Message::Prevote(Vote::new(s.round, value))))
}

// received a complete proposal for an empty or invalid value - prevote nil
// 22
fn handle_proposal_invalid(s: State) -> (State, Option<Message>){
    let s = s.set_step(RoundStep::Prevote);
    (s, Some(Message::Prevote(Vote::new(s.round, None))))
}

// received a complete proposal with old (polka) value - prevote
// 28
fn handle_proposal_polka(s: State, vr: i64, proposed: Value) -> (State, Option<Message>) {
    let s = s.set_step(RoundStep::Prevote);
    let value = match s.locked {
        Some(locked) if locked.round <= vr => { Some(proposed) } // unlock and prevote
        Some(locked) if locked.value == proposed => { Some(proposed) } // already locked on value
        _ => { None } // otherwise, prevote nil
    };
    (s, Some(Message::Prevote(Vote::new(s.round, value))))
}

// timed out of propose - prevote nil
// 57
fn handle_timeout_propose(s: State) -> (State, Option<Message>) {
    let s = s.set_step(RoundStep::Prevote);
    (s, Some(Message::Prevote(Vote::new(s.round, None))))
}

// 34
// NOTE: this should only be called once in a round, per the spec,
// but it's harmless to schedule more timeouts
fn handle_polka_any(s: State) -> (State, Option<Message>) {
    (s, Some(Message::Timeout(Timeout::new(s.round, RoundStep::Prevote))))
}

// 44
fn handle_polka_nil(s: State) -> (State, Option<Message>) {
    let s = s.set_step(RoundStep::Precommit);
    (s, Some(Message::Precommit(Vote::new(s.round, None))))
}

// 36
// NOTE: only one of these two funcs should ever be called, and only once, in a round
fn handle_polka_value_prevote(s: State, v: Value) -> (State, Option<Message>) {
    let s = s.set_locked(v).set_valid(v).set_step(RoundStep::Precommit);
    (s, Some(Message::Precommit(Vote::new(s.round, Some(v)))))
}

// 36/42
fn handle_polka_value_precommit(s: State, v: Value) -> (State, Option<Message>) {
    let s = s.set_valid(v);
    (s, None)
}

// 61
fn handle_timeout_prevote(s: State) -> (State, Option<Message>) {
    let s = s.set_step(RoundStep::Precommit);
    (s, Some(Message::Precommit(Vote::new(s.round, None))))
}

// 47
fn handle_precommit_any(s: State) -> (State, Option<Message>) {
    (s, Some(Message::Timeout(Timeout::new(s.round, RoundStep::Precommit))))
}

// 49
fn handle_precommit_value(s: State, r: i64, v: Value) -> (State, Option<Message>) {
    let s = s.set_step(RoundStep::Commit);
    (s, Some(Message::Decision(RoundValue{round: r, value: v})))
}

// 65
fn handle_timeout_precommit(s: State, r: i64) -> (State, Option<Message>) {
    let s = s.set_step(RoundStep::Precommit);
    (s, Some(Message::Precommit(Vote::new(s.round, None))))
}




fn main() {
    println!("Hello, world!");
}
