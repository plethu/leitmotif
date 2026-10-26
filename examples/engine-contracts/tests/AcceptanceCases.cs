using System.Reflection;
using System.Runtime.Versioning;
using System.Text.Json.Nodes;
using Harbour;

public static class AcceptanceCases
{
    public static void Run(string directory, Func<string[], int>? consume = null)
    {
        consume ??= args => new Consumer().Run(args);
        var framework = typeof(Consumer)
            .Assembly.GetCustomAttribute<TargetFrameworkAttribute>()
            ?.FrameworkName;
        if (framework != ".NETStandard,Version=v2.1")
            throw new InvalidOperationException($"unexpected Shared artifact target: {framework}");
        var requestPath = Path.Combine(directory, "sheltered.request.json");
        var reportPath = Path.Combine(directory, "sheltered.check.json");
        var request = JsonNode.Parse(File.ReadAllText(requestPath))!;
        var report = JsonNode.Parse(File.ReadAllText(reportPath))!;
        var cases = report["conformance"]!["cases"]!.AsArray();
        var fixedAuditWork = cases.Sum(item => item!["prefix"]!.AsArray().Count + 1);
        if (
            report["conformance"]!["states"]!.GetValue<int>() != 6
            || cases.Count != 30
            || fixedAuditWork != 85
        )
            throw new InvalidOperationException("fixed six-state audit cost changed");
        var next = 0;
        void Check(JsonNode input, JsonNode output, int expected)
        {
            var stem = Path.Combine(directory, $"negative-{next++}");
            File.WriteAllText(stem + ".request.json", input.ToJsonString());
            File.WriteAllText(stem + ".check.json", output.ToJsonString());
            var actual = consume([stem + ".request.json", stem + ".check.json"]);
            if (actual != expected)
                throw new InvalidOperationException(
                    $"case {next}: expected {expected}, got {actual}"
                );
        }
        void Input(Action<JsonNode> mutate)
        {
            var copy = request.DeepClone();
            mutate(copy);
            Check(copy, report, 2);
        }
        void Output(Action<JsonNode> mutate, int expected = 2)
        {
            var copy = report.DeepClone();
            mutate(copy);
            Check(request, copy, expected);
        }
        foreach (
            var (name, expected) in new[]
            {
                ("sheltered", 0),
                ("departed", 0),
                ("unreachable", 1),
                ("inconclusive", 2),
            }
        )
        {
            var status = consume([
                Path.Combine(directory, name + ".request.json"),
                Path.Combine(directory, name + ".check.json"),
            ]);
            if (status != expected)
                throw new InvalidOperationException($"{name}: {status}");
        }
        foreach (
            var key in new[]
            {
                "schema_version",
                "binding_version",
                "model_id",
                "source_revision",
                "query",
            }
        )
            Input(x => x[key] = "invalid");
        Input(x => x["source_revision"] = " ");
        Input(x => x["extra"] = true);
        foreach (var fact in request["initial"]!.AsObject().Select(item => item.Key))
        {
            Input(x => x["initial"]!.AsObject().Remove(fact));
            Input(x => x["initial"]![fact] = 17);
        }
        Input(x => x["initial"]!["harbour.ending"] = "unknown");
        Input(x => x["initial"]!["extra"] = false);
        Input(x => x["initial"]!["harbour.oil_available"] = false);
        foreach (var limit in new[] { "max_states", "max_depth" })
        {
            Input(x => x["limits"]![limit] = -1);
            Output(x => x["analysis"]!["report"]!["coverage"]!["limits"]![limit] = 17);
        }
        Output(x =>
        {
            x["conformance"]!["states"] = 0;
            x["conformance"]!["cases"] = new JsonArray();
        });
        Output(x =>
        {
            x["conformance"]!["states"] = 5;
            var cases = x["conformance"]!["cases"]!.AsArray();
            while (cases.Count > 25)
                cases.RemoveAt(25);
        });
        Output(x => x["kind"] = "inconclusive");
        Output(x => x["conformance"]!["states"] = 858993466);
        Output(x => x["analysis"]!["report"]!["target"]!["expression"]!["operation"] = "not");
        Output(x =>
            x["analysis"]!["report"]!["target"]!["expression"]!["arguments"]!["fact"] =
                "keeper.warning_shared"
        );
        Output(x =>
            x["analysis"]!["report"]!["target"]!["expression"]!["arguments"]!["value"]!["type"] =
                "bool"
        );
        Output(x =>
            x["analysis"]!["report"]!["target"]!["expression"]!["arguments"]!["value"]!["value"] =
                "departed"
        );
        foreach (
            var (name, field, value) in new[]
            {
                ("unreachable", "exhausted", false),
                ("unreachable", "state_limit_hit", true),
                ("unreachable", "depth_limit_hit", true),
                ("inconclusive", "exhausted", true),
                ("inconclusive", "depth_limit_hit", false),
            }
        )
        {
            var originalRequest = JsonNode.Parse(
                File.ReadAllText(Path.Combine(directory, name + ".request.json"))
            )!;
            var changedReport = JsonNode.Parse(
                File.ReadAllText(Path.Combine(directory, name + ".check.json"))
            )!;
            changedReport["analysis"]!["report"]!["coverage"]![field] = value;
            Check(originalRequest, changedReport, 2);
        }
        Output(x => x["schema_version"] = "future");
        Output(x => x["kind"] = "unsupported");
        foreach (
            var key in new[] { "schema_version", "binding_version", "source_revision", "query" }
        )
            Output(x => x["analysis"]![key] = "invalid");
        Output(x => x["analysis"]!["report"]!["schema_version"] = "future");
        Output(x => x["analysis"]!["report"]!["model_id"] = "tampered");
        Output(x =>
            x["analysis"]!["report"]!["initial"]!["values"]!["harbour.oil_available"]!["value"] =
                false
        );
        Output(x => x["analysis"]!["report"]!["conclusion"]!["kind"] = "unknown");
        Output(x =>
            x["analysis"]!["report"]!["conclusion"]!["witness"]!["initial"]!["values"]![
                "harbour.oil_available"
            ]!["value"] = false
        );
        Output(x =>
            x["analysis"]!["report"]!["conclusion"]!["witness"]!["steps"]![0]!["transition"]![
                "id"
            ] = "unknown"
        );
        Output(
            x =>
                x["analysis"]!["report"]!["conclusion"]!["witness"]!["steps"]![0]!["transition"]![
                    "id"
                ] = "harbour.depart",
            1
        );
        Output(
            x =>
                x["analysis"]!["report"]!["conclusion"]!["witness"]!["steps"]![0]!["state"]![
                    "values"
                ]!["keeper.warning_shared"]!["value"] = false,
            1
        );
        Output(
            x =>
                x["analysis"]!["report"]!["conclusion"]!["witness"]!["steps"]!
                    .AsArray()
                    .RemoveAt(2),
            1
        );
        Output(x => x["conformance"]!["cases"]!.AsArray().RemoveAt(0));
        Output(x => x["conformance"]!["cases"]![0] = x["conformance"]!["cases"]![1]!.DeepClone());
        Output(x => x["conformance"]!["cases"]![0]!["action"] = "unknown");
        Output(x => x["conformance"]!["cases"]![0]!["prefix"]!.AsArray().Add("harbour.depart"), 1);
        Output(
            x =>
                x["conformance"]!["cases"]![0]!["before"]!["values"]!["keeper.warning_shared"]![
                    "value"
                ] = true,
            1
        );
        Output(x => x["conformance"]!["cases"]![0]!["expected_enabled"] = false, 1);
        Output(
            x =>
                x["conformance"]!["cases"]![0]!["expected"]!["values"]!["harbour.oil_available"]![
                    "value"
                ] = false,
            1
        );
        Output(x =>
            x["conformance"]!["cases"]![0]!["expected"]!["values"]!["harbour.oil_available"]![
                "type"
            ] = "symbol"
        );
        if (
            consume([]) != 2
            || consume(["absent", "absent"]) != 2
            || consume([directory, reportPath]) != 2
        )
            throw new InvalidOperationException("argument/file rejection failed");
        Console.WriteLine(
            $"Consumer acceptance: four actual Rust outcomes and {next} adversarial cases passed"
        );
    }
}
