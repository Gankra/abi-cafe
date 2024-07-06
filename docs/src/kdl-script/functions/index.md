# functions

Functions are where the Actually Useful *library* version of KDLScript and the Just A Meme *application* version of KDLScript diverge. This difference is configured by the `eval` feature.

As a library, KDLScript only has [function *signature declarations*](./signatures.md), and it's the responsibility of the ABI Cafe backend using KDLScript to figure out what the body should be.

As a CLI binary, KDLScript [actually lets you fill in the body with some hot garbage I hacked up](./bodies.md).

