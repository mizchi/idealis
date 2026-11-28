import os, sys, shutil
tree, mode = sys.argv[1], sys.argv[2]
assert mode in ('before', 'after')
S = os.environ['S']

def sub(path, old, new, count=1):
    path = os.path.join(tree, path)
    s = open(path).read()
    assert s.count(old) == count, (path, old[:70], s.count(old))
    open(path, 'w').write(s.replace(old, new))

shutil.copy(os.path.join(S, 'probe.rs.txt'), os.path.join(tree, 'crates/rtlc/src/probe.rs'))
sub('crates/rtlc/src/lib.rs', "pub mod ir;\n", "pub mod ir;\npub mod probe;\n")
e = 'crates/rtlc/src/safety_expressions.rs'
if mode == 'before':
    sub(e, """        if self.evaluation_depth >= RECURSIVE_DEPTH_LIMIT {
            if self.recursive {
                return Err(CoreError {
                    code: "evaluation-budget",
                    message: "Recursive safety evaluation nesting exceeds 128".into(),
                });
            }
            if self.evaluation_depth >= DEPTH_LIMIT {
                return Err(CoreError::protocol("Safety evaluation nesting exceeds 256"));
            }
        }
        self.evaluation_depth += 1;""", """        crate::probe::record(self.evaluation_depth, self.evaluation_depth);
        if self.evaluation_depth >= RECURSIVE_DEPTH_LIMIT {
            if self.recursive {
                return Err(CoreError {
                    code: "evaluation-budget",
                    message: "Recursive safety evaluation nesting exceeds 128".into(),
                });
            }
            if self.evaluation_depth >= DEPTH_LIMIT {
                crate::probe::hit();
                if self.evaluation_depth >= crate::probe::LIMIT {
                    return Err(CoreError::protocol("Safety evaluation nesting exceeds 256"));
                }
            }
        }
        self.evaluation_depth += 1;""")
else:
    sub(e, """    fn refuse_excess_nesting(&self) -> Result<(), CoreError> {
        if self.nesting_depth() >= RECURSIVE_DEPTH_LIMIT {
            if self.recursive {
                return Err(CoreError {
                    code: "evaluation-budget",
                    message: "Recursive safety evaluation nesting exceeds 128".into(),
                });
            }
            if self.evaluation_depth >= DEPTH_LIMIT {
                return Err(CoreError::protocol("Safety evaluation nesting exceeds 256"));
            }
        }
        Ok(())
    }""", """    fn refuse_excess_nesting(&self) -> Result<(), CoreError> {
        crate::probe::record(self.evaluation_depth, self.nesting_depth());
        if self.nesting_depth() >= RECURSIVE_DEPTH_LIMIT {
            if self.recursive {
                return Err(CoreError {
                    code: "evaluation-budget",
                    message: "Recursive safety evaluation nesting exceeds 128".into(),
                });
            }
            if self.evaluation_depth >= DEPTH_LIMIT {
                crate::probe::hit();
                if self.evaluation_depth >= crate::probe::LIMIT {
                    return Err(CoreError::protocol("Safety evaluation nesting exceeds 256"));
                }
            }
        }
        Ok(())
    }""")
sub(e, """                result = body
                    .eval(
                        EvalContext {
                            locals: &locals,
                            ..ctx
                        },
                        arena,
                    )
                    .map_err(|error| match replacement {""", """                let entered = crate::probe::fold_enter(replacement);
                let evaluated = body.eval(
                    EvalContext {
                        locals: &locals,
                        ..ctx
                    },
                    arena,
                );
                crate::probe::fold_exit(entered);
                result = evaluated
                    .map_err(|error| match replacement {""")

p = 'crates/rtlc/src/safety_parallel.rs'
sub(p, """    fn push(&mut self, parent: usize, action: usize, values: &[Option<Scalar>]) {
        self.events.push(Event::Candidate {""", """    fn push(&mut self, parent: usize, action: usize, values: &[Option<Scalar>]) {
        crate::probe::worker_note(self.events.len());
        self.events.push(Event::Candidate {""")
sub(p, """struct Buffer {
    candidates: Candidates,
    next: Vec<Option<Scalar>>,
}""", """struct Buffer {
    candidates: Candidates,
    next: Vec<Option<Scalar>>,
    cut: Option<(usize, String)>,
}""")
sub(p, """            candidates: Candidates::default(),
            next: vec![None; width],
        }""", """            candidates: Candidates::default(),
            next: vec![None; width],
            cut: None,
        }""")
sub(p, """        self.candidates.clear();
        self.next.fill(None);""", """        self.candidates.clear();
        self.next.fill(None);
        self.cut = None;""")
sub(p, """            arena.work += expansions.iter().map(|delta| delta.work).sum::<u32>();""", """            crate::probe::mark_parallel();
            if crate::probe::cutting() {
                for buffer in workers.buffers.iter_mut().take(expansions.len()) {
                    if let Some((length, message)) = buffer.cut.take() {
                        buffer.candidates.events.truncate(length);
                        buffer
                            .candidates
                            .events
                            .push(Event::Error(CoreError::protocol(&message)));
                    }
                }
            }
            arena.work += expansions.iter().map(|delta| delta.work).sum::<u32>();""")
sub(p, """    fn expand_range(
        &self,
        range: Range<usize>,
        base: &Arena,
        budget: &WorkBudget,
        worker: usize,
        buffer: &mut Buffer,
    ) -> ValueDelta {
        buffer.reset();""", """    fn expand_range(
        &self,
        range: Range<usize>,
        base: &Arena,
        budget: &WorkBudget,
        worker: usize,
        buffer: &mut Buffer,
    ) -> ValueDelta {
        crate::probe::worker_begin();
        let delta = self.expand_range_observed(range, base, budget, worker, buffer);
        buffer.cut = crate::probe::worker_end(buffer.candidates.events.len());
        delta
    }

    fn expand_range_observed(
        &self,
        range: Range<usize>,
        base: &Arena,
        budget: &WorkBudget,
        worker: usize,
        buffer: &mut Buffer,
    ) -> ValueDelta {
        buffer.reset();""")
sub(p, """                    Err(error) => {
                        buffer.candidates.events.push(Event::Error(error));""", """                    Err(error) => {
                        crate::probe::worker_note(buffer.candidates.events.len());
                        buffer.candidates.events.push(Event::Error(error));""")
sub(p, """            if !enabled && self.program.check_deadlock {
                buffer.candidates.events.push(Event::Deadlock(parent));""", """            if !enabled && self.program.check_deadlock {
                crate::probe::worker_note(buffer.candidates.events.len());
                buffer.candidates.events.push(Event::Deadlock(parent));""")

s = 'crates/rtlc/src/safety.rs'
sub(s, """        self.check_measured::<NoCoverage>(
            bounds,
            SafetyOptions {
                workers,
                ..Default::default()
            },
            None,
            None,
            cache,
        )
    }""", """        let options = SafetyOptions {
            workers,
            ..Default::default()
        };
        if cache.is_some() {
            return self.check_measured::<NoCoverage>(bounds, options, None, None, cache);
        }
        crate::probe::observed(|| {
            self.check_measured::<NoCoverage>(bounds, options, None, None, None)
        })
    }""")
sub(s, """        self.validate_options(options)?;
        self.check_measured::<NoCoverage>(bounds, options, None, None, None)
    }""", """        self.validate_options(options)?;
        crate::probe::observed(|| {
            self.check_measured::<NoCoverage>(bounds, options, None, None, None)
        })
    }""")

c = 'crates/idealis-core/src/lib.rs'
sub(c, """    pub fn handle(&mut self, command: Command) -> Message {""", """    pub fn dispatch_observed(&mut self, bytes: &[u8]) -> Vec<u8> {
        rtlc::probe::begin();
        let message = self.dispatch(bytes);
        let summary = rtlc::probe::summary();
        let published = match summary.published {
            rtlc::probe::Published::Own => None,
            rtlc::probe::Published::Hit(message) => Some(Message::Error {
                code: "protocol".into(),
                message,
            }),
            rtlc::probe::Published::Cut(Ok(result)) => Some(Message::Safety { result }),
            rtlc::probe::Published::Cut(Err((code, message))) => Some(Message::Error {
                code: code.into(),
                message,
            }),
        };
        let published = match published {
            Some(message) => serde_json::to_vec(&message).expect("serializable core message"),
            None => b"null".to_vec(),
        };
        let mut probe = format!(
            "{{\\"hits256\\":{},\\"peakDepth\\":{},\\"peakTotal\\":{},\\"published256\\":",
            summary.hits, summary.peak_depth, summary.peak_total
        )
        .into_bytes();
        probe.extend_from_slice(&published);
        probe.push(b'}');
        let mut output = serde_json::to_vec(&message).expect("serializable core message");
        assert_eq!(output.pop(), Some(b'}'));
        output.extend_from_slice(b",\\"probe\\":");
        output.extend_from_slice(&probe);
        output.push(b'}');
        output
    }
    pub fn handle(&mut self, command: Command) -> Message {""")
sub('crates/idealis-core/src/wasm.rs', """        let message = instance.engine.dispatch(&instance.input);
        instance.output = serde_json::to_vec(&message).expect("serializable core message");""", """        instance.output = instance.engine.dispatch_observed(&instance.input);""")
sub('crates/idealis-core/src/main.rs', """        println!(
            "{}",
            serde_json::to_string(&engine.dispatch(line?.as_bytes()))?
        );""", """        println!(
            "{}",
            String::from_utf8_lossy(&engine.dispatch_observed(line?.as_bytes()))
        );""")
