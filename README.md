# Structure-Aware PSI
This is a Rust implementation for the Structure Aware PSI protocol in the paper *New Framework for Structure-Aware PSI From Distributed Function Secret Sharing*: https://eprint.iacr.org/2025/907.pdf.

# How to install
The code is well tested for Rust 1.84.0. Simply git clone this repository and build it with --release.

# Experimental settings

As we do not run this protocol multithreaded, the sender and the receiver are ran on the same machine, with enough number of cores so that the two parties do not fight for resources.
To simulate WAN, we use the Linux command tc.

# How to use
To run the project: 

1. Change the sa/src/config.rs file to set up the parameters:
    - DIMENSION: Choose the number of dimensions. We tested on DIMENSION = 2, 3, 4.
    - RADIUS_PARAM_CHOOSE: Choose the radius along with our chosen parameters. Currently we support:
        - 0: RADIUS = 10 
        - 1: RADIUS = 30 
        - 2: RADIUS = 60 
        - 3: RADIUS = 120 
        - 4: RADIUS = 250
    - N: Choose the set size 2^N. Currently we support N = 8, 12, 16.

Note that, the config.rs file on the machine that runs the sender should be identical to the config.rs file on the machine that runs the receiver.
We will support runtime config in the future.

2. Run the following codes on two terminal tabs (or on two different machines). Note that, in this code, the receiver is the Tcp Listener, the code for receiver should be ran first.
Support for retrying to connect to Tcp will be added in the future.
> cargo run --release --bin psi [role] [port]

Examples for running receiver/sender:
> cargo run --release --bin psi receiver 8080 </br>
> cargo run --release --bin psi sender 8080 

# Components of the code
## aes
This directory contains some toolkit for PRG.
In the implementation of ibDCF for this paper, we only utilize prg.rs, and handle the tree expansion manually.

## network
We implement messages sending/receiving with Tcp.
Messages are sent sequentially, which, does not fully utilize the bandwidth in LAN (compared to multithreading).
However, in WAN settigns, the current code is sufficient to utilize the low bandwidth.

## okvs
We implement RB-OKVS: https://www.usenix.org/system/files/usenixsecurity23-bienstock.pdf.

The implementation follows an open-source repository: https://github.com/felicityin/rb-okvs.git.


## ot 
Our implementation is based on the C++ implementation in EMP-toolkit: https://github.com/emp-toolkit/emp-ot.git.

We implement the base OT protocol from SimplestOT: https://eprint.iacr.org/2015/267.pdf.

We implement the OT extension protocol from IKNP: https://www.iacr.org/archive/crypto2003/27290145/27290145.pdf.

To convert from random OT to message OT, we follow section 6.1 here: https://eprint.iacr.org/2019/074.pdf.

## sa

This directory contains the code for Structure-Aware PSI and the implementation of Distributed DCF.
- *ibDCF*. In simple terms, we need to implement a GGM tree. We use fixed-key AES to expand a node into its children. 
- *SA-PSI*. For more details of the protocol, please refer to our paper. 

## utils 
This directory contains a simple implementation of the field GF(128). This implementation naively does multiplication, so a lot of optimization (such as SIMD instructions) can be done here.

# Choosing parameters for LPN in VOLE extension
We modify the published code of the following paper: https://eprint.iacr.org/2022/712.pdf.
We add multiprocess to speed up the Python script provided, and publish the code at: https://github.com/phuocchubeo123/LPN-Estimator-Multiprocess.git.

In the process of running the code, we also found a bug. When running home/estimator.py, one must be careful with the command for regular noise. 
Putting "regular" will actually compute the exact noise instead, since the current regex script detects the letter e. To run the script for regular noise, one could use "rgular" instead.