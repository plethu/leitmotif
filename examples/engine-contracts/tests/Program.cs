using Harbour;

AcceptanceCases.Run(args[0]);
var output = new List<string>();
var errors = new List<string>();
var consumer = new Consumer(output.Add, errors.Add);
AcceptanceCases.Run(args[0], consumer.Run);
if (
    !output.Any(line => line.Contains("Harbour conformance: 30 cases"))
    || !errors.Any(line => line.Contains("Rejected harbour evidence:"))
    || !errors.Any(line => line.Contains("examples/support/composed_town/"))
)
    throw new InvalidOperationException(
        "consumer diagnostic sinks lost conformance or source evidence"
    );
