//! LambdaCodec — pure lambda-calculus Codec (SPEC-27 R10-R16).
//!
//! Encodes terms in the minimal grammar `Term ::= Var | Lam | App` as
//! IC nets via the Mackie/Pinto pipeline (REF-005, Section 5):
//! - Lambda → CON (p0=output, p1=binder, p2=body)
//! - Application → CON (p0=function, p1=result, p2=argument)
//! - Variable used n times → DUP tree of (n−1) duplicators rooted at the binder
//! - Variable used 0 times → ERA on the binder port
//! - Free variable → `FreePort(fresh_id)` connection
//!
//! Decoding uses port-directed readback (R14): the entry port at
//! `FreePort(0)` is the result; CON-via-p0 is a Lambda, CON-via-p1 is an
//! Application result, ERA is an erased branch, DUP is followed via its
//! principal source.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::traits::{Codec, DecodeError, Decoder, EncodeError, Encoder};
use crate::net::{AgentId, Net, PortRef, Symbol};

/// A pure lambda-calculus term (SPEC-27 R11).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Term {
    Var(String),
    Lam(String, Box<Term>),
    App(Box<Term>, Box<Term>),
}

/// Codec for the pure lambda calculus (SPEC-27 R10).
#[derive(Debug, Clone, Copy, Default)]
pub struct LambdaCodec;

impl LambdaCodec {
    pub fn new() -> Self {
        Self
    }
}

// ============================================================================
// Parser — hand-written recursive descent (SPEC-27 R12, Q3)
// ============================================================================

/// Parse a textual lambda term. Accepts `λ`, `\` and `lambda` as binders.
/// Application is left-associative; parentheses override precedence.
pub fn parse_term(input: &str) -> Result<Term, String> {
    let mut p = Parser { input, pos: 0 };
    let t = p.parse_term()?;
    p.skip_ws();
    if p.pos != input.len() {
        return Err(format!("trailing input at byte {}", p.pos));
    }
    Ok(t)
}

struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn rest(&self) -> &str {
        &self.input[self.pos..]
    }

    fn skip_ws(&mut self) {
        while let Some(c) = self.rest().chars().next() {
            if c.is_whitespace() {
                self.pos += c.len_utf8();
            } else {
                break;
            }
        }
    }

    fn consume_str(&mut self, s: &str) -> bool {
        if self.rest().starts_with(s) {
            self.pos += s.len();
            true
        } else {
            false
        }
    }

    fn parse_ident(&mut self) -> Result<String, String> {
        self.skip_ws();
        let start = self.pos;
        match self.rest().chars().next() {
            Some(c) if c.is_ascii_alphabetic() || c == '_' => {
                self.pos += c.len_utf8();
            }
            _ => return Err(format!("expected identifier at byte {}", self.pos)),
        }
        while let Some(c) = self.rest().chars().next() {
            if c.is_ascii_alphanumeric() || c == '_' {
                self.pos += c.len_utf8();
            } else {
                break;
            }
        }
        Ok(self.input[start..self.pos].to_string())
    }

    /// Recognise the `lambda` keyword only if the following char is a non-ident
    /// boundary (so we don't tokenise `lambdax` as `lambda` + `x`).
    fn at_lambda_keyword(&self) -> bool {
        let r = self.rest();
        if !r.starts_with("lambda") {
            return false;
        }
        match r["lambda".len()..].chars().next() {
            None => true,
            Some(c) => !c.is_ascii_alphanumeric() && c != '_',
        }
    }

    fn parse_atom(&mut self) -> Result<Term, String> {
        self.skip_ws();
        let r = self.rest();
        if r.starts_with('λ') {
            self.pos += 'λ'.len_utf8();
            self.parse_binder_tail()
        } else if r.starts_with('\\') {
            self.pos += 1;
            self.parse_binder_tail()
        } else if self.at_lambda_keyword() {
            self.pos += "lambda".len();
            self.parse_binder_tail()
        } else if r.starts_with('(') {
            self.pos += 1;
            let t = self.parse_term()?;
            self.skip_ws();
            if !self.consume_str(")") {
                return Err(format!("expected ')' at byte {}", self.pos));
            }
            Ok(t)
        } else {
            let name = self.parse_ident()?;
            Ok(Term::Var(name))
        }
    }

    fn parse_binder_tail(&mut self) -> Result<Term, String> {
        let name = self.parse_ident()?;
        self.skip_ws();
        if !self.consume_str(".") {
            return Err(format!("expected '.' after binder at byte {}", self.pos));
        }
        let body = self.parse_term()?;
        Ok(Term::Lam(name, Box::new(body)))
    }

    fn parse_term(&mut self) -> Result<Term, String> {
        let mut left = self.parse_atom()?;
        loop {
            self.skip_ws();
            let r = self.rest();
            if r.is_empty() || r.starts_with(')') {
                break;
            }
            let right = self.parse_atom()?;
            left = Term::App(Box::new(left), Box::new(right));
        }
        Ok(left)
    }
}

// ============================================================================
// Pretty printer
// ============================================================================

pub fn print_term(t: &Term) -> String {
    let mut out = String::new();
    print_into(t, &mut out, /*app_ctx=*/ false);
    out
}

fn print_into(t: &Term, out: &mut String, app_ctx: bool) {
    match t {
        Term::Var(name) => out.push_str(name),
        Term::Lam(name, body) => {
            if app_ctx {
                out.push('(');
            }
            out.push('λ');
            out.push_str(name);
            out.push_str(". ");
            print_into(body, out, false);
            if app_ctx {
                out.push(')');
            }
        }
        Term::App(f, a) => {
            // Lambda-on-left needs parens — otherwise the binder's body
            // would extend over the argument under standard scope rules.
            let f_needs_paren = matches!(**f, Term::Lam(_, _));
            if f_needs_paren {
                out.push('(');
            }
            print_into(f, out, false);
            if f_needs_paren {
                out.push(')');
            }
            out.push(' ');
            // Right side needs parens for any App or Lam (left-assoc app
            // and longest-scope lambda would otherwise bind the wrong tree).
            let a_needs_paren = matches!(**a, Term::App(_, _) | Term::Lam(_, _));
            if a_needs_paren {
                out.push('(');
            }
            print_into(a, out, false);
            if a_needs_paren {
                out.push(')');
            }
        }
    }
}

// ============================================================================
// JSON I/O envelopes (SPEC-27 R12, R15)
// ============================================================================

#[derive(Debug, Deserialize)]
struct LambdaInput {
    #[serde(default)]
    term: Option<String>,
    #[serde(default)]
    ast: Option<Term>,
}

#[derive(Debug, Serialize)]
struct LambdaOutput {
    term: String,
    agents: usize,
    interactions: serde_json::Value,
}

// ============================================================================
// Encoder (REF-005 mapping, SPEC-27 R13)
// ============================================================================

/// Count uses of `name` in `term`, respecting binder shadowing.
fn count_uses(term: &Term, name: &str) -> usize {
    match term {
        Term::Var(n) => (n == name) as usize,
        Term::Lam(x, body) => {
            if x == name {
                0
            } else {
                count_uses(body, name)
            }
        }
        Term::App(f, a) => count_uses(f, name) + count_uses(a, name),
    }
}

/// Build a binder fan-out chain rooted at `root`.
///
/// - `n == 0`: connect an ERA to `root`, return no leaves.
/// - `n == 1`: return `root` itself as the single leaf.
/// - `n >= 2`: build a left-biased chain of (n-1) DUP agents and return the
///   `n` leaf ports in left-to-right order.
fn build_binder_chain(net: &mut Net, root: PortRef, n: usize) -> Vec<PortRef> {
    if n == 0 {
        let era = net.create_agent(Symbol::Era);
        net.connect(root, PortRef::AgentPort(era, 0));
        return Vec::new();
    }
    if n == 1 {
        return vec![root];
    }
    let mut leaves = Vec::with_capacity(n);
    let mut current = root;
    for _ in 0..(n - 1) {
        let dup = net.create_agent(Symbol::Dup);
        net.connect(current, PortRef::AgentPort(dup, 0));
        leaves.push(PortRef::AgentPort(dup, 1));
        current = PortRef::AgentPort(dup, 2);
    }
    leaves.push(current);
    leaves
}

struct EncodeCtx {
    /// Binder name → stack of leaf-port queues (stack handles shadowing).
    /// Each queue is consumed back-to-front so leaves[0] goes to the first
    /// in-order use of the variable.
    bindings: HashMap<String, Vec<Vec<PortRef>>>,
    next_free_port: u32,
}

impl EncodeCtx {
    fn new() -> Self {
        Self {
            bindings: HashMap::new(),
            // FreePort(0) is reserved as the top-level result wire.
            next_free_port: 1,
        }
    }

    fn push_binding(&mut self, name: &str, mut leaves: Vec<PortRef>) {
        // Reverse so .pop() yields leaves in their original (left-to-right) order.
        leaves.reverse();
        self.bindings
            .entry(name.to_string())
            .or_default()
            .push(leaves);
    }

    fn pop_binding(&mut self, name: &str) {
        if let Some(stack) = self.bindings.get_mut(name) {
            stack.pop();
            if stack.is_empty() {
                self.bindings.remove(name);
            }
        }
    }

    fn consume_leaf(&mut self, name: &str) -> Option<PortRef> {
        let stack = self.bindings.get_mut(name)?;
        let top = stack.last_mut()?;
        top.pop()
    }

    fn fresh_free_port(&mut self) -> u32 {
        let f = self.next_free_port;
        self.next_free_port += 1;
        f
    }
}

fn encode_term(net: &mut Net, term: &Term, ctx: &mut EncodeCtx) -> PortRef {
    match term {
        Term::Var(name) => {
            if let Some(leaf) = ctx.consume_leaf(name) {
                leaf
            } else {
                // Free variable — surface as FreePort.
                PortRef::FreePort(ctx.fresh_free_port())
            }
        }
        Term::Lam(x, body) => {
            let lam = net.create_agent(Symbol::Con);
            let n = count_uses(body, x);
            let leaves = build_binder_chain(net, PortRef::AgentPort(lam, 1), n);
            ctx.push_binding(x, leaves);
            let body_port = encode_term(net, body, ctx);
            ctx.pop_binding(x);
            net.connect(body_port, PortRef::AgentPort(lam, 2));
            PortRef::AgentPort(lam, 0)
        }
        Term::App(f, a) => {
            let app = net.create_agent(Symbol::Con);
            let f_port = encode_term(net, f, ctx);
            let a_port = encode_term(net, a, ctx);
            net.connect(f_port, PortRef::AgentPort(app, 0));
            net.connect(a_port, PortRef::AgentPort(app, 2));
            PortRef::AgentPort(app, 1)
        }
    }
}

/// Encode a term into a fresh net. Top-level result is wired to FreePort(0).
pub fn encode_lambda(term: &Term) -> Net {
    let mut net = Net::new();
    let mut ctx = EncodeCtx::new();
    let result = encode_term(&mut net, term, &mut ctx);
    net.connect(result, PortRef::FreePort(0));
    net
}

impl Encoder for LambdaCodec {
    fn name(&self) -> &str {
        "lambda"
    }

    fn encode(&self, input: &[u8]) -> Result<Net, EncodeError> {
        let env: LambdaInput = serde_json::from_slice(input)
            .map_err(|e| EncodeError::InvalidInput(format!("JSON parse failed: {}", e)))?;

        let term = match (env.term, env.ast) {
            (Some(s), _) => {
                parse_term(&s).map_err(|e| EncodeError::InvalidInput(format!("parse: {}", e)))?
            }
            (None, Some(ast)) => ast,
            (None, None) => {
                return Err(EncodeError::InvalidInput(
                    "missing 'term' or 'ast' field".to_string(),
                ));
            }
        };

        Ok(encode_lambda(&term))
    }
}

// ============================================================================
// Decoder — port-directed readback (SPEC-27 R14, R15)
// ============================================================================

/// Find the agent port whose target is `FreePort(fid)`.
fn find_entry_port(net: &Net, fid: u32) -> Option<PortRef> {
    for agent in net.live_agents() {
        for p in 0u8..3 {
            let port = PortRef::AgentPort(agent.id, p);
            if net.get_target(port) == PortRef::FreePort(fid) {
                return Some(port);
            }
        }
    }
    None
}

struct ReadbackCtx {
    var_map: HashMap<(AgentId, u8), String>,
    counter: usize,
    /// Cycle guard: cap recursion depth to avoid infinite loops on
    /// pathological DUP cycles (a known open problem for optimal readback).
    depth: usize,
}

const MAX_READBACK_DEPTH: usize = 10_000;

impl ReadbackCtx {
    fn new() -> Self {
        Self {
            var_map: HashMap::new(),
            counter: 0,
            depth: 0,
        }
    }

    fn fresh_name(&mut self) -> String {
        let n = format!("v{}", self.counter);
        self.counter += 1;
        n
    }
}

fn readback(net: &Net, port: PortRef, ctx: &mut ReadbackCtx) -> Result<Term, DecodeError> {
    if ctx.depth >= MAX_READBACK_DEPTH {
        return Err(DecodeError::UnrecognizedStructure(
            "readback exceeded depth limit (likely DUP cycle)".to_string(),
        ));
    }
    ctx.depth += 1;
    let result = readback_inner(net, port, ctx);
    ctx.depth -= 1;
    result
}

fn readback_inner(net: &Net, port: PortRef, ctx: &mut ReadbackCtx) -> Result<Term, DecodeError> {
    let (id, p) = match port {
        PortRef::AgentPort(id, p) => (id, p),
        PortRef::FreePort(fid) => {
            return Ok(Term::Var(format!("free_{}", fid)));
        }
    };

    if let Some(name) = ctx.var_map.get(&(id, p)).cloned() {
        return Ok(Term::Var(name));
    }

    let agent = net
        .get_agent(id)
        .ok_or_else(|| DecodeError::UnrecognizedStructure(format!("dead agent {}", id)))?;

    match (agent.symbol, p) {
        (Symbol::Con, 0) => {
            // Lambda: p1 = binder, p2 = body.
            let name = ctx.fresh_name();
            ctx.var_map.insert((id, 1), name.clone());
            let body_target = net.get_target(PortRef::AgentPort(id, 2));
            let body = readback(net, body_target, ctx)?;
            ctx.var_map.remove(&(id, 1));
            Ok(Term::Lam(name, Box::new(body)))
        }
        (Symbol::Con, 1) => {
            // Application result port: p0 = function, p2 = argument.
            let f_target = net.get_target(PortRef::AgentPort(id, 0));
            let a_target = net.get_target(PortRef::AgentPort(id, 2));
            let f = readback(net, f_target, ctx)?;
            let a = readback(net, a_target, ctx)?;
            Ok(Term::App(Box::new(f), Box::new(a)))
        }
        (Symbol::Con, 2) => Err(DecodeError::UnrecognizedStructure(format!(
            "unexpected entry into CON agent {} via aux port p2",
            id
        ))),
        (Symbol::Era, _) => Ok(Term::Var("_".to_string())),
        (Symbol::Dup, _) => {
            // We are reading back one copy of a duplicated value. Follow the
            // DUP's principal source for the original. In a normal-form
            // result of T5-T9 this terminates; pathological DUP cycles trip
            // the depth guard above.
            let src = net.get_target(PortRef::AgentPort(id, 0));
            readback(net, src, ctx)
        }
        (_, _) => Err(DecodeError::UnrecognizedStructure(format!(
            "unhandled {:?}:{}",
            agent.symbol, p
        ))),
    }
}

/// Decode a normal-form lambda net to a Term.
pub fn decode_lambda(net: &Net) -> Result<Term, DecodeError> {
    let entry = find_entry_port(net, 0).ok_or_else(|| {
        DecodeError::UnrecognizedStructure("no FreePort(0) entry port".to_string())
    })?;
    let mut ctx = ReadbackCtx::new();
    readback(net, entry, &mut ctx)
}

impl Decoder for LambdaCodec {
    fn decode(&self, net: &Net) -> Result<serde_json::Value, DecodeError> {
        let term = decode_lambda(net)?;
        let out = LambdaOutput {
            term: print_term(&term),
            agents: net.count_live_agents(),
            // Pipeline driver fills this in (Phase 5); not known at decode time.
            interactions: serde_json::Value::Null,
        };
        serde_json::to_value(out).map_err(|e| DecodeError::DecodeFailed(e.to_string()))
    }
}

impl Codec for LambdaCodec {
    fn description(&self) -> &str {
        "Pure lambda-calculus (encode, reduce, port-directed readback)"
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reduction::reduce_all;

    fn count_sym(net: &Net, s: Symbol) -> usize {
        net.live_agents().filter(|a| a.symbol == s).count()
    }

    /// Canonicalise a term by renaming bound variables to a fixed sequence.
    /// Free variables keep their names. Used for alpha-equivalence checks.
    fn canonicalise(t: &Term) -> Term {
        fn go(t: &Term, env: &mut Vec<(String, String)>, c: &mut usize) -> Term {
            match t {
                Term::Var(n) => {
                    let resolved = env
                        .iter()
                        .rev()
                        .find_map(|(orig, new)| if orig == n { Some(new.clone()) } else { None })
                        .unwrap_or_else(|| n.clone());
                    Term::Var(resolved)
                }
                Term::Lam(n, body) => {
                    let new_name = format!("#{}", *c);
                    *c += 1;
                    env.push((n.clone(), new_name.clone()));
                    let b = go(body, env, c);
                    env.pop();
                    Term::Lam(new_name, Box::new(b))
                }
                Term::App(f, a) => Term::App(Box::new(go(f, env, c)), Box::new(go(a, env, c))),
            }
        }
        let mut env = Vec::new();
        let mut c = 0;
        go(t, &mut env, &mut c)
    }

    fn alpha_equiv(a: &str, b: &str) -> bool {
        let ta = parse_term(a).expect("parse a");
        let tb = parse_term(b).expect("parse b");
        canonicalise(&ta) == canonicalise(&tb)
    }

    // --- Parser tests (P1-P5 from TEST-SPEC-0333) ---

    #[test]
    fn parse_identity() {
        let t = parse_term("λx. x").unwrap();
        match t {
            Term::Lam(n, b) => {
                assert_eq!(n, "x");
                assert!(matches!(*b, Term::Var(ref v) if v == "x"));
            }
            _ => panic!("expected Lam"),
        }
    }

    #[test]
    fn parse_application_left_associative() {
        let t = parse_term("a b c").unwrap();
        // (a b) c — outer App's f is itself App
        match t {
            Term::App(f, _) => assert!(matches!(*f, Term::App(_, _))),
            _ => panic!("expected App"),
        }
    }

    #[test]
    fn parse_alternative_lambda_syntax() {
        let a = parse_term("\\x. x").unwrap();
        let b = parse_term("lambda x. x").unwrap();
        let c = parse_term("λx. x").unwrap();
        assert_eq!(a, b);
        assert_eq!(b, c);
    }

    #[test]
    fn parse_parens_override_precedence() {
        let t = parse_term("a (b c)").unwrap();
        match t {
            Term::App(_, a) => assert!(matches!(*a, Term::App(_, _))),
            _ => panic!("expected App"),
        }
    }

    #[test]
    fn parse_error_returns_invalid_input() {
        let codec = LambdaCodec::new();
        let err = codec.encode(r#"{"term":"(λx."}"#.as_bytes()).unwrap_err();
        assert!(matches!(&err, EncodeError::InvalidInput(m) if m.contains("parse")));
    }

    // --- Encoder tests (E1-E6) ---

    #[test]
    fn encode_identity_structure() {
        let codec = LambdaCodec::new();
        let net = codec.encode(r#"{"term":"λx. x"}"#.as_bytes()).unwrap();
        // λx.x is one CON with p1↔p2 self-loop.
        assert_eq!(count_sym(&net, Symbol::Con), 1);
        assert_eq!(count_sym(&net, Symbol::Dup), 0);
        assert_eq!(count_sym(&net, Symbol::Era), 0);
        assert!(net.redex_queue.is_empty());
    }

    #[test]
    fn encode_beta_creates_redex() {
        let codec = LambdaCodec::new();
        let net = codec
            .encode(r#"{"term":"(λx. x) (λy. y)"}"#.as_bytes())
            .unwrap();
        assert!(
            !net.redex_queue.is_empty(),
            "(λx.x)(λy.y) must contain a CON-CON redex"
        );
    }

    #[test]
    fn encode_unused_binder_produces_era() {
        let codec = LambdaCodec::new();
        let net = codec.encode(r#"{"term":"λx. λy. y"}"#.as_bytes()).unwrap();
        assert!(count_sym(&net, Symbol::Era) >= 1, "unused x must be erased");
    }

    #[test]
    fn encode_shared_variable_produces_dup() {
        let codec = LambdaCodec::new();
        let net = codec.encode(r#"{"term":"λx. x x"}"#.as_bytes()).unwrap();
        assert!(
            count_sym(&net, Symbol::Dup) >= 1,
            "x used twice must introduce a DUP"
        );
    }

    #[test]
    fn encode_accepts_json_ast_input() {
        let codec = LambdaCodec::new();
        let json = br#"{"ast":{"Lam":["x",{"Var":"x"}]}}"#;
        let net = codec.encode(json).unwrap();
        assert_eq!(count_sym(&net, Symbol::Con), 1);
    }

    #[test]
    fn codec_name_and_description() {
        let codec = LambdaCodec::new();
        assert_eq!(codec.name(), "lambda");
        assert!(codec.description().to_lowercase().contains("lambda"));
    }

    // --- Pretty-printer (PP1-PP3) ---

    #[test]
    fn pretty_print_nested_lambda() {
        let t = parse_term("λx. λy. x").unwrap();
        assert_eq!(print_term(&t), "λx. λy. x");
    }

    #[test]
    fn pretty_print_app_parens_when_right_is_app() {
        let t = parse_term("a (b c)").unwrap();
        assert_eq!(print_term(&t), "a (b c)");
    }

    #[test]
    fn pretty_print_round_trip_structural() {
        let t1 = parse_term("(λf. λx. f (f x)) (λy. y)").unwrap();
        let s = print_term(&t1);
        let t2 = parse_term(&s).unwrap();
        assert_eq!(t1, t2);
    }

    // --- Object safety (O1, EC1) ---

    #[test]
    fn lambda_codec_object_safe() {
        let boxed: Box<dyn Codec> = Box::new(LambdaCodec::new());
        assert_eq!(boxed.name(), "lambda");
        let _ = boxed.description();
    }

    // --- Round-trip edge cases (T5-T9 from SPEC-27 §6.3) ---

    #[test]
    fn t5_identity_round_trip() {
        let codec = LambdaCodec::new();
        let net = codec.encode(r#"{"term":"λx. x"}"#.as_bytes()).unwrap();
        // Identity is already in normal form — decode without reducing.
        let out = codec.decode(&net).unwrap();
        let s = out["term"].as_str().unwrap();
        assert!(alpha_equiv(s, "λx. x"), "got {}", s);
    }

    #[test]
    fn t6_single_beta_reduction() {
        let codec = LambdaCodec::new();
        let mut net = codec
            .encode(r#"{"term":"(λx. x) (λy. y)"}"#.as_bytes())
            .unwrap();
        reduce_all(&mut net);
        let out = codec.decode(&net).unwrap();
        let s = out["term"].as_str().unwrap();
        assert!(alpha_equiv(s, "λy. y"), "got {}", s);
    }

    #[test]
    fn t7_nested_double_identity() {
        let codec = LambdaCodec::new();
        let mut net = codec
            .encode(r#"{"term":"(λf. λx. f (f x)) (λy. y)"}"#.as_bytes())
            .unwrap();
        reduce_all(&mut net);
        let out = codec.decode(&net).unwrap();
        let s = out["term"].as_str().unwrap();
        assert!(alpha_equiv(s, "λx. x"), "got {}", s);
    }

    #[test]
    fn t8_erasure() {
        let codec = LambdaCodec::new();
        let mut net = codec
            .encode(r#"{"term":"(λx. λy. y) (λz. z)"}"#.as_bytes())
            .unwrap();
        reduce_all(&mut net);
        let out = codec.decode(&net).unwrap();
        let s = out["term"].as_str().unwrap();
        assert!(alpha_equiv(s, "λy. y"), "got {}", s);
    }

    #[test]
    fn t9_duplication() {
        let codec = LambdaCodec::new();
        let mut net = codec
            .encode(r#"{"term":"(λx. x x) (λy. y)"}"#.as_bytes())
            .unwrap();
        reduce_all(&mut net);
        let out = codec.decode(&net).unwrap();
        let s = out["term"].as_str().unwrap();
        assert!(alpha_equiv(s, "λy. y"), "got {}", s);
    }

    // --- Negative tests (N1-N3) ---

    #[test]
    fn n1_malformed_json() {
        let codec = LambdaCodec::new();
        let err = codec.encode(b"{not json}").unwrap_err();
        assert!(matches!(err, EncodeError::InvalidInput(_)));
    }

    #[test]
    fn n2_missing_body_after_dot() {
        let codec = LambdaCodec::new();
        let err = codec.encode(r#"{"term":"(λx."}"#.as_bytes()).unwrap_err();
        assert!(matches!(&err, EncodeError::InvalidInput(m) if m.contains("parse")));
    }

    #[test]
    fn n3_decode_foreign_net_structure() {
        let codec = LambdaCodec::new();
        let mut foreign = Net::new();
        foreign.create_agent(Symbol::Era);
        let err = codec.decode(&foreign).unwrap_err();
        assert!(matches!(err, DecodeError::UnrecognizedStructure(_)));
    }

    // --- Decoder is read-only (D4) ---

    #[test]
    fn decoder_does_not_mutate_net() {
        let codec = LambdaCodec::new();
        let net = codec.encode(r#"{"term":"λx. x"}"#.as_bytes()).unwrap();
        let snapshot = net.clone();
        let _ = codec.decode(&net).unwrap();
        assert_eq!(net, snapshot, "decode must not mutate input net");
    }

    // --- Output schema (D5) ---

    #[test]
    fn decode_output_has_required_fields() {
        let codec = LambdaCodec::new();
        let net = codec.encode(r#"{"term":"λx. x"}"#.as_bytes()).unwrap();
        let out = codec.decode(&net).unwrap();
        assert!(out.get("term").is_some());
        assert!(out.get("agents").is_some());
        assert!(out.get("interactions").is_some());
    }

    // --- Missing 'term' or 'ast' field ---

    #[test]
    fn encode_rejects_missing_fields() {
        let codec = LambdaCodec::new();
        let err = codec.encode(b"{}").unwrap_err();
        assert!(matches!(&err, EncodeError::InvalidInput(m) if m.contains("missing")));
    }

    // --- QA (Stage 5) adversarial cases ---

    /// QA-1: shadowed binder — inner `x` rebinds, outer is unused.
    /// Encoding must not unify the two `x`s. Expected count_uses on the outer
    /// binder = 0 (because inner `Lam("x", ...)` shadows it).
    #[test]
    fn qa_shadowed_binder_does_not_leak() {
        assert_eq!(count_uses(&parse_term("λx. λx. x").unwrap(), "x"), 0);
        let codec = LambdaCodec::new();
        let net = codec.encode(r#"{"term":"λx. λx. x"}"#.as_bytes()).unwrap();
        // Outer x is unused → ERA on its binder slot.
        assert!(count_sym(&net, Symbol::Era) >= 1);
    }

    /// QA-2: deeply nested left-associative application parses without recursion blow-up.
    #[test]
    fn qa_deep_left_assoc_parses() {
        let t = parse_term("a b c d e f g h").unwrap();
        // Verify left-associativity: outermost shape is App(_, Var("h")).
        match t {
            Term::App(_, ref last) => assert!(matches!(**last, Term::Var(ref n) if n == "h")),
            _ => panic!("expected App at outer level"),
        }
    }

    /// QA-3: alternative lambda syntaxes parse to identical structures.
    #[test]
    fn qa_alternative_syntaxes_agree() {
        let t1 = parse_term("\\x. x").unwrap();
        let t2 = parse_term("lambda x. x").unwrap();
        let t3 = parse_term("λx. x").unwrap();
        assert_eq!(t1, t3);
        assert_eq!(t2, t3);
    }

    /// QA-4: pretty-print round-trip is stable on a non-trivial term.
    #[test]
    fn qa_print_parse_idempotent() {
        let s = "λf. λx. f (f x)";
        let t = parse_term(s).unwrap();
        let s2 = print_term(&t);
        let t2 = parse_term(&s2).unwrap();
        assert_eq!(t, t2);
    }

    /// QA-5: identity applied to identity reduces to identity (chained).
    #[test]
    fn qa_chained_identity_reduces() {
        let codec = LambdaCodec::new();
        let mut net = codec
            .encode(r#"{"term":"((λx. x) (λx. x)) (λy. y)"}"#.as_bytes())
            .unwrap();
        reduce_all(&mut net);
        let out = codec.decode(&net).unwrap();
        let s = out["term"].as_str().unwrap();
        assert!(alpha_equiv(s, "λy. y"), "got {}", s);
    }

    /// QA-6: free variable in encoding output is preserved as a FreePort wire.
    #[test]
    fn qa_free_variable_uses_freeport() {
        let codec = LambdaCodec::new();
        let net = codec.encode(r#"{"term":"f x"}"#.as_bytes()).unwrap();
        // `f` and `x` are both free → at least 2 fresh FreePorts (id ≥ 1)
        // are wired into the App CON.
        let app = net.live_agents().find(|a| a.symbol == Symbol::Con).unwrap();
        let p0 = net.get_target(PortRef::AgentPort(app.id, 0));
        let p2 = net.get_target(PortRef::AgentPort(app.id, 2));
        assert!(matches!(p0, PortRef::FreePort(fid) if fid >= 1));
        assert!(matches!(p2, PortRef::FreePort(fid) if fid >= 1));
    }
}
