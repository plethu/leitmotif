using System.Text.Json;

namespace Harbour;

public sealed class Consumer
{
    // This executable host declares its own initial world; it never hydrates it from a report/request.
    private readonly HarbourState initial = new(Ending.Open, true, false, false, false, false);
    private readonly ContractChecks host;

    public Consumer() => host = new(initial);

    public int Run(string[] args)
    {
        try
        {
            Wire.Require(args.Length == 2, "expected request.json and checked.json");
            using var request = JsonDocument.Parse(File.ReadAllText(args[0]));
            using var result = JsonDocument.Parse(File.ReadAllText(args[1]));
            var evidence = Evidence.Decode(request.RootElement, result.RootElement, initial);
            var status = host.Check(evidence);
            Console.WriteLine(
                $"Harbour conformance: {evidence.Cases.Length} cases; {evidence.Query}: {evidence.Verdict.ToString().ToLowerInvariant()} (status {status})"
            );
            return status;
        }
        catch (HostDifference error)
        {
            Console.Error.WriteLine($"Harbour conformance failure: {error.Message}");
            return 1;
        }
        catch (Exception error)
            when (error
                    is Rejected
                        or JsonException
                        or InvalidOperationException
                        or KeyNotFoundException
                        or IOException
                        or UnauthorizedAccessException
                        or FormatException
                        or OverflowException
            )
        {
            Console.Error.WriteLine($"Rejected harbour evidence: {error.Message}");
            return 2;
        }
    }
}
