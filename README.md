# `CppEnjoyers` implementation of [`TextServer`] and [`MediaServer`]

 Supports compression of packets (if requested by client).
 Available compressions are:
 - Huffman
 - LZW

 The [`GenericServer`] uses ETX estimation to decide the best routing paths.

 The estimator uses an exponentially weighted moving average (EWMA),
 the formula is as follows:
 ``` text
     ETX(n) = p(n)  alpha + ETX(n - 1)  beta
     ETX(0) = default_etx_value
 ```
 where:
 - ETX(n) is the ETX at time n
 - p(n) is the estimated ETX at time n, calculated from the last k samples (k is a predefined constant)
 - alpha and beta are parameters that decide how fast the ETX adapts to change

 # High level protocol

 The protocol between Client and Server is defined as follows:
 - Upon discorvering a Server node the Client can query it to discover its type
 - The server answers with a response containing its type
 - After that the Client can query the Server for a list of available file or for a specific file
 - The Server answer with the requested information or with an error in case of unknown/unsupported requests

 Every request is associated with a request id (16 bits) that will be part of the response id used by the server.
 In this way the Client can easily recognise the request associated with the response and handle it accordingly.

 Every request/response is serialized and fragmented into binary before being sent as packets in the netowork.
 Optionally the Client can specify in the request a compression method to use on the serialized data: this
 can help reduce the network bottleneck due to less packets being sent.
