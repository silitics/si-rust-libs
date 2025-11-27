
For debugging and reporting:

1. Errors MUST characterize the operation that failed.
2. Errors MUST characterize why the operation failed.
3. Errors SHOULD suggest how to fix the cause of the error.
4. Errors MAY be structured and machine-readable.

This information is targeted at human readers. It does not need to be machine-readable and, in fact, it SHOULD NOT be interpreted by machines in any way as it may leak implementation details that are not part of an explicit API contract.

*Context:* Where in the program execution did the error occur? Two ways to attach context: (i) when the error is produced (backtrace, span trace) or (ii) whenever the error is propagated. There is some nuance here as a backtrace does not contain variable values while those can be explicitly added on propagation.

Adding context comes with overhead. If the error is handled somehow, i.e., not reported, this context is not necessary. Although, it may still be good to have it, even when handling an error, just so to log the information that there was an error.


For handling:

1. Errors MAY characterize the operation that failed.
2. Errors SHOULD characterize why the operation failed.
3. Errors MUST be structured and machine-readable.

For handling, `std::io::Error` is fine as it fulfills 2 and 3. It doesn't fulfil 1 as it doesn't include the operation that failed. Note that this makes it unsuitable for error reporting. For reporting, we need to add additional context characterizing the operation that failed (like reading or opening a file or sending data over a socket). The caller has this context. If you propagate it without adding context about the operation that failed, error messages will be useless for debugging and understanding what went wrong.

For handling, errors form part of the public API contract and should be given the same thought as any public interface. It is a valid design decision to not expose certain errors individually. For instance, `load_json` may decide to allow callers to handle reading and deserialization errors, but, one may also decide to just return an opaque `LoadError`.
