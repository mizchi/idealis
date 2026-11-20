---- MODULE ParallelChannels ----
EXTENDS Naturals
CONSTANT N
VARIABLE chan
Init == chan = [node \in 1..N |-> [val |-> 0, rdy |-> 0, ack |-> 0]]
Send(node,message) == /\ chan[node].rdy=chan[node].ack
                      /\ chan'=[chan EXCEPT ![node].val=message, ![node].rdy=1-@]
Rcv(node) == /\ chan[node].rdy # chan[node].ack
             /\ chan'=[chan EXCEPT ![node].ack=1-@]
Next == \E node \in 1..N: (\E message \in {0,1}: Send(node,message)) \/ Rcv(node)
Safe == (chan \in [1..N -> [val:{0,1}, rdy:{0,1}, ack:{0,1}]]) /\ (\A node \in 1..N: chan[node].val=0)
====