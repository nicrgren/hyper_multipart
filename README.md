# Multipart body parsing of hyper responses

## Examples
To run the examples, the env STREAM_URL must be set. To see some output, set env RUST_LOG={example}=debug


# TODO
Some camera streams seems to be implicit multipart. I.e they send a continous stream of jpeg responses.
This is easily handled by looking at the response header. If the header is non multipart we just continously parse
multipart chunks from it. 
Make sure Hyper does not already do this.....
