# N.E.E.B.L.E.S. Boss 1.0.27

Boss 1.0.27 hardens CUSTOM V2 raw-material transport discovered during the first Fresh Live acceptance attempt after Point 1 + Point 2 closure.

Fresh Live successfully passed the previously failing module-material territory provisioning, resolved the immutable Test Module revision and entered Preinstall. The next failure exposed a transport bug: Debian package filenames containing literal percent-encoded epochs such as `%3a` were inserted directly into a raw GitHub URL. HTTP URL decoding changed that physical filename identity before GitHub lookup, producing HTTP 404.

This release adds and certifies:

- segment-aware CUSTOM raw URL construction using the existing `url::Url` dependency;
- exact 40-character hexadecimal CUSTOM revision validation at the raw transport boundary;
- explicit rejection of empty, current-directory, parent-directory and control-character path segments;
- preservation of literal `%xx` text as filename identity rather than URL syntax;
- protection against encoded slash and encoded traversal becoming path structure;
- protection against filename `?` and `#` becoming query or fragment components;
- safe transport of spaces, backslashes, Unicode and reserved URL delimiters as segment data;
- unchanged package SHA, membership, Essential/module-delta ownership and MaterialBinding semantics.

Certification completed before release:

- 116 / 116 Boss library tests GREEN;
- 594 / 594 Boss backend tests GREEN before the exhaustive extension;
- all-target cargo check GREEN;
- 91 / 91 current package URLs resolve from immutable CUSTOM revision;
- all 8 current literal `%3a` packages download with exact declared SHA256;
- control probe reproduces old HTTP 404 and new HTTP 200 behavior;
- 94 printable ASCII filename-character cases GREEN;
- 512 literal `%00` through `%FF` upper/lower cases GREEN;
- 33 ASCII controls rejected fail-closed;
- 151,739 sensitive-token single/pair/triple combinations round-trip exactly;
- 16 / 16 directed URL transport tests GREEN.

Certified CUSTOM revision:

20488f6818d5e227f043425a682115f614f4bf79

Certified Test Module revision:

bbd2e43d7f3a4f8ab30a913be7589478fb796863

Point 1 and Point 2 remain CLOSED / GREEN at source level.

Fresh Live and installed-system acceptance remain separate system-level gates and must be re-run with Boss 1.0.27.
