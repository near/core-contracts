# Voting Contract

The purpose of this contract is solely for validators to vote on whether to unlock
token transfer. Validators can call `vote` to vote for yes with the amount of stake they wish
to put on the vote. If there are more than 2/3 of the stake at any given moment voting for yes, the voting is done.
After the voting is finished, no one can further modify the contract.
