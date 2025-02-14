# PICS
This is a Rust implementation for the Private Intersection over Committed Sets (PICS) protocol in the paper *Does Malicious Security Suffice for Private Set Intersection?*

# How to use
The project contains many bin files that is used to test each small part.
For running the PSI protocol as a proof-of-work, please use the following command:
> cargo run --release --bin oprf (role) (ip) (port) (log_set_size) (num_threads) (lpn_param) (committed)

Currently, we support three sets of LPN parameters: LPN17, LPN21, LPN25 (correspondingly, lpn_param=0,1,2), which are suitable for running $2^{17}$, $2^{21}$, and $2^{25}$ VOLEs.

# This code adapts from
1. Wolverine: https://github.com/emp-toolkit/emp-zk.git
2. RB-OKVS: https://github.com/felicityin/rb-okvs.git
3. VOLE-PSI: https://github.com/Visa-Research/volepsi.git
4. lambdaworks: https://github.com/lambdaclass/lambdaworks.git
