---- MODULE NondetTags_TTrace_1795166512 ----
EXTENDS Sequences, TLCExt, Toolbox, Naturals, TLC, NondetTags

_expression ==
    LET NondetTags_TEExpression == INSTANCE NondetTags_TEExpression
    IN NondetTags_TEExpression!expression
----

_trace ==
    LET NondetTags_TETrace == INSTANCE NondetTags_TETrace
    IN NondetTags_TETrace!trace
----

_inv ==
    ~(
        TLCGet("level") = Len(_TETrace)
        /\
        last = ("reply")
        /\
        pending = ({})
        /\
        done = (3)
    )
----

_init ==
    /\ done = _TETrace[1].done
    /\ pending = _TETrace[1].pending
    /\ last = _TETrace[1].last
----

_next ==
    /\ \E i,j \in DOMAIN _TETrace:
        /\ \/ /\ j = i + 1
              /\ i = TLCGet("level")
        /\ done  = _TETrace[i].done
        /\ done' = _TETrace[j].done
        /\ pending  = _TETrace[i].pending
        /\ pending' = _TETrace[j].pending
        /\ last  = _TETrace[i].last
        /\ last' = _TETrace[j].last

\* Uncomment the ASSUME below to write the states of the error trace
\* to the given file in Json format. Note that you can pass any tuple
\* to `JsonSerialize`. For example, a sub-sequence of _TETrace.
    \* ASSUME
    \*     LET J == INSTANCE Json
    \*         IN J!JsonSerialize("NondetTags_TTrace_1795166512.json", _TETrace)

=============================================================================

 Note that you can extract this module `NondetTags_TEExpression`
  to a dedicated file to reuse `expression` (the module in the 
  dedicated `NondetTags_TEExpression.tla` file takes precedence 
  over the module `NondetTags_TEExpression` below).

---- MODULE NondetTags_TEExpression ----
EXTENDS Sequences, TLCExt, Toolbox, Naturals, TLC, NondetTags

expression == 
    [
        \* To hide variables of the `NondetTags` spec from the error trace,
        \* remove the variables below.  The trace will be written in the order
        \* of the fields of this record.
        done |-> done
        ,pending |-> pending
        ,last |-> last
        
        \* Put additional constant-, state-, and action-level expressions here:
        \* ,_stateNumber |-> _TEPosition
        \* ,_doneUnchanged |-> done = done'
        
        \* Format the `done` variable as Json value.
        \* ,_doneJson |->
        \*     LET J == INSTANCE Json
        \*     IN J!ToJson(done)
        
        \* Lastly, you may build expressions over arbitrary sets of states by
        \* leveraging the _TETrace operator.  For example, this is how to
        \* count the number of times a spec variable changed up to the current
        \* state in the trace.
        \* ,_doneModCount |->
        \*     LET F[s \in DOMAIN _TETrace] ==
        \*         IF s = 1 THEN 0
        \*         ELSE IF _TETrace[s].done # _TETrace[s-1].done
        \*             THEN 1 + F[s-1] ELSE F[s-1]
        \*     IN F[_TEPosition - 1]
    ]

=============================================================================



Parsing and semantic processing can take forever if the trace below is long.
 In this case, it is advised to uncomment the module below to deserialize the
 trace from a generated binary file.

\*
\*---- MODULE NondetTags_TETrace ----
\*EXTENDS IOUtils, TLC, NondetTags
\*
\*trace == IODeserialize("NondetTags_TTrace_1795166512.bin", TRUE)
\*
\*=============================================================================
\*

---- MODULE NondetTags_TETrace ----
EXTENDS TLC, NondetTags

trace == 
    <<
    ([last |-> "init",pending |-> {},done |-> 0]),
    ([last |-> "request",pending |-> {1},done |-> 0]),
    ([last |-> "request",pending |-> {1, 2},done |-> 0]),
    ([last |-> "request",pending |-> {1, 2, 3},done |-> 0]),
    ([last |-> "reply",pending |-> {2, 3},done |-> 1]),
    ([last |-> "reply",pending |-> {3},done |-> 2]),
    ([last |-> "reply",pending |-> {},done |-> 3])
    >>
----


=============================================================================

---- CONFIG NondetTags_TTrace_1795166512 ----

INVARIANT
    _inv

CHECK_DEADLOCK
    \* CHECK_DEADLOCK off because of PROPERTY or INVARIANT above.
    FALSE

INIT
    _init

NEXT
    _next

CONSTANT
    _TETrace <- _trace

ALIAS
    _expression
=============================================================================
\* Generated on Fri Nov 20 18:21:53 JST 2026