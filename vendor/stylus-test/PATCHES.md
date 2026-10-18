# Local test framework patch

This directory vendors `stylus-test` 0.10.10 from the signed SDK release
commit `76f8b055ec0bf421edd542b164dc2917c9e4e0bd` (the crate is dual licensed
under MIT or Apache-2.0; see `LICENSE-MIT` and `LICENSE-APACHE`).

The local patch changes only mocked call behavior. SDK 0.10.10 stored each
mock's response in shared `return_data` when the mock was registered, so every
external call read whichever response had been registered last. The EVM
exposes the response from the call that just ran. This patch publishes the
matched response after each call and does not alter production SDK or contract
code. The regression is covered by
`contracts/tests/testvm_mock_return_data.rs`.
