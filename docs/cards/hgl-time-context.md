# Card: hgl-time-context

Status: accepted

Host-only construction context. Uses hgl-time-values, hgl-literals, hgl-types
and Jiff; budget 200 source lines. Jiff is approved for this implementation and
is declared once under workspace.dependencies. Use system zone data on Unix
and bundled platform data on Windows through its default provider configuration.
Enable tzdb-bundle-always so generated conformance tests can explicitly select
the same bundled catalog on every platform, independently of host packaging.

RunContext exposes from_env() -> Result<Self, String>,
from_bundled() -> Result<Self, String> for the conformance test host,
from_database(jiff::tz::TimeZoneDatabase) -> Result<Self, String>, and
materialize(&mut self, &TemporalLiteral) -> Result<Literal, String>.
The explicit database constructor permits controlled native host tests.

Snapshot the provider's exact catalog during construction. Require case-sensitive
membership before backend lookup; never inherit case folding or synthetic unknown
zones. For zoned datetimes, verify the supplied offset against the provider at the
supplied instant. Zoned times validate the wall-clock range and exact catalog
name without resolving a date-dependent offset. Preserve the original name and reject lookup, range or offset
failures without fallback/truncation. The compiler never invokes this provider.

The context owns provider state outside graph ticks. Already constructed scalar
copies, replay and record never resolve again. No HGL injectable, process-global
provider or new source configuration API is introduced. Test exact aliases,
missing/wrong-case names, offset mismatch, range failure and retained identity.
