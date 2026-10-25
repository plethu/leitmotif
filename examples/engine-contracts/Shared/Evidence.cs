using System.Text.Json;

namespace Harbour;

internal enum Verdict
{
    Reached,
    Unreachable,
    Inconclusive,
}

internal sealed record AuditCase(
    HarbourAction[] Prefix,
    HarbourState Before,
    HarbourAction Action,
    string Source,
    bool Enabled,
    HarbourState Expected
);

internal sealed record ReplayStep(HarbourAction Action, HarbourState Expected);

internal sealed record Evidence(
    AuditCase[] Cases,
    Verdict Verdict,
    ReplayStep[] Steps,
    string Query
)
{
    public static Evidence Decode(JsonElement request, JsonElement result, HarbourState initial)
    {
        Wire.Shape(
            request,
            "schema_version",
            "model_id",
            "binding_version",
            "source_revision",
            "query",
            "limits",
            "initial"
        );
        Wire.Require(
            request.GetProperty("schema_version").GetString() == Wire.Schema,
            "unsupported request schema"
        );
        Wire.Require(
            request.GetProperty("binding_version").GetString() == Wire.Binding,
            "unsupported binding"
        );
        Wire.Require(
            request.GetProperty("model_id").GetString() == Wire.Model,
            "unsupported model"
        );
        Wire.Require(
            !string.IsNullOrWhiteSpace(request.GetProperty("source_revision").GetString()),
            "empty source revision"
        );
        Wire.Require(
            Wire.State(request.GetProperty("initial"), false) == initial,
            "request differs from live host initial state"
        );
        var limits = request.GetProperty("limits");
        Wire.Shape(limits, "max_states", "max_depth");
        Wire.Require(limits.GetProperty("max_states").GetInt32() >= 0, "invalid state limit");
        Wire.Require(limits.GetProperty("max_depth").GetInt32() >= 0, "invalid depth limit");
        var query = request.GetProperty("query").GetString();
        Wire.Require(query is "sheltered" or "departed" or "storm_passed", "unsupported query");
        Wire.Require(
            result.GetProperty("schema_version").GetString() == Wire.Schema,
            "unsupported response schema"
        );
        Wire.Require(
            result.GetProperty("kind").GetString() is "checked" or "inconclusive",
            "analysis did not produce a checkable report"
        );
        var analysis = result.GetProperty("analysis");
        foreach (
            var key in new[] { "schema_version", "binding_version", "source_revision", "query" }
        )
            Wire.Require(
                analysis.GetProperty(key).GetString() == request.GetProperty(key).GetString(),
                "report identity differs"
            );
        var report = analysis.GetProperty("report");
        Wire.Require(
            report.GetProperty("schema_version").GetString() == "leitmotif.experimental.v2",
            "unsupported report schema"
        );
        Wire.Require(
            report.GetProperty("model_id").GetString() == Wire.Model,
            "report model differs"
        );
        Wire.Require(
            Wire.Snapshot(report.GetProperty("initial")) == initial,
            "report initial differs"
        );
        var reportLimits = report.GetProperty("coverage").GetProperty("limits");
        foreach (var key in new[] { "max_states", "max_depth" })
            Wire.Require(
                reportLimits.GetProperty(key).GetInt32() == limits.GetProperty(key).GetInt32(),
                "report limits differ"
            );
        var target = report.GetProperty("target").GetProperty("expression");
        Wire.Require(
            target.GetProperty("operation").GetString() == "eq",
            "unsupported report target"
        );
        var arguments = target.GetProperty("arguments");
        Wire.Require(
            arguments.GetProperty("fact").GetString()
                == (query == "storm_passed" ? "harbour.storm_passed" : "harbour.ending"),
            "report target fact differs"
        );
        var targetValue = arguments.GetProperty("value");
        Wire.Shape(targetValue, "type", "value");
        Wire.Require(
            targetValue.GetProperty("type").GetString()
                == (query == "storm_passed" ? "bool" : "symbol"),
            "report target type differs"
        );
        if (query == "storm_passed")
            Wire.Require(
                targetValue.GetProperty("value").GetBoolean(),
                "report target value differs"
            );
        else
            Wire.Require(
                targetValue.GetProperty("value").GetString() == query,
                "report target value differs"
            );
        var conclusion = report.GetProperty("conclusion");
        var verdict = conclusion.GetProperty("kind").GetString() switch
        {
            "reached" => Verdict.Reached,
            "unreachable" => Verdict.Unreachable,
            "inconclusive" => Verdict.Inconclusive,
            _ => throw new Rejected("unsupported conclusion"),
        };
        Wire.Require(
            (result.GetProperty("kind").GetString() == "inconclusive")
                == (verdict == Verdict.Inconclusive),
            "outer verdict differs"
        );
        var coverage = report.GetProperty("coverage");
        var exhausted = coverage.GetProperty("exhausted").GetBoolean();
        var stateLimit = coverage.GetProperty("state_limit_hit").GetBoolean();
        var depthLimit = coverage.GetProperty("depth_limit_hit").GetBoolean();
        Wire.Require(
            verdict != Verdict.Unreachable || (exhausted && !stateLimit && !depthLimit),
            "unreachable report is not exhaustive"
        );
        Wire.Require(
            verdict != Verdict.Inconclusive || (!exhausted && (stateLimit || depthLimit)),
            "inconclusive report has inconsistent limits"
        );
        ReplayStep[] steps = [];
        if (verdict == Verdict.Reached)
        {
            var witness = conclusion.GetProperty("witness");
            Wire.Require(
                Wire.Snapshot(witness.GetProperty("initial")) == initial,
                "witness initial differs"
            );
            steps = witness
                .GetProperty("steps")
                .EnumerateArray()
                .Select(step => new ReplayStep(
                    Wire.Action(step.GetProperty("transition").GetProperty("id").GetString()),
                    Wire.Snapshot(step.GetProperty("state"))
                ))
                .ToArray();
        }
        var audit = result.GetProperty("conformance");
        var cases = audit
            .GetProperty("cases")
            .EnumerateArray()
            .Select(item => new AuditCase(
                item.GetProperty("prefix")
                    .EnumerateArray()
                    .Select(id => Wire.Action(id.GetString()))
                    .ToArray(),
                Wire.Snapshot(item.GetProperty("before")),
                Wire.Action(item.GetProperty("action").GetString()),
                item.GetProperty("provenance").GetProperty("portable_file").GetString()
                    ?? throw new Rejected("missing action source"),
                item.GetProperty("expected_enabled").GetBoolean(),
                Wire.Snapshot(item.GetProperty("expected"))
            ))
            .ToArray();
        Wire.Require(audit.GetProperty("states").GetInt32() == 6, "incomplete action audit");
        Wire.Require(cases.Length == 30, "unsupported harbour audit size");
        return new(cases, verdict, steps, query!);
    }
}
