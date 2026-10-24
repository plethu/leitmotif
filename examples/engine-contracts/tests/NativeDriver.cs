using System.Text;
using Godot;
using Harbour;
using Dictionary = Godot.Collections.Dictionary;

public partial class NativeDriver : Node
{
    private HarbourState state = Initial();
    private string fault = "";
    private int resets;
    private int faultProbes;
    private int controlledProbes;
    private int captures;
    private byte[] asset = [];

    private static HarbourState Initial() => new(Ending.Open, true, false, false, false, false);

    public void Reset()
    {
        state = Initial();
        resets++;
    }

    public Variant Capture()
    {
        captures++;
        var facts = new Dictionary
        {
            ["harbour.ending"] = state.Ending.ToString().ToLowerInvariant(),
            ["harbour.oil_available"] = state.Oil,
            ["harbour.storm_passed"] = state.Storm,
            ["keeper.warning_shared"] = state.Warning,
            ["keeper.beacon_lit"] = state.Beacon,
            ["ferryman.ready"] = state.Ferry,
        };
        switch (fault)
        {
            case "missing":
                facts.Remove("harbour.oil_available");
                break;
            case "extra":
                facts["unknown"] = true;
                break;
            case "unknown-key":
                facts.Remove("harbour.oil_available");
                facts["unknown"] = true;
                break;
            case "wrong-type":
                facts["harbour.oil_available"] = "true";
                break;
            case "wrong-symbol-type":
                facts["harbour.ending"] = 1;
                break;
            case "unknown-ending":
                facts["harbour.ending"] = "gone";
                break;
            case "stale":
                facts["harbour.oil_available"] = false;
                break;
        }
        if ((fault == "query-wrong-type" && captures >= 6))
            facts["harbour.oil_available"] = "true";
        if (
            (fault == "query-drift" && captures >= 6) || (fault == "request-drift" && captures >= 3)
        )
            facts["harbour.oil_available"] = false;
        return fault == "wrong-capture" ? Variant.From(1) : Variant.From(facts);
    }

    public Variant Perform(string action)
    {
        var mapped = action switch
        {
            "keeper.share_warning" => HarbourAction.ShareWarning,
            "keeper.light_beacon" => HarbourAction.LightBeacon,
            "ferryman.fuel_ferry" => HarbourAction.FuelFerry,
            "harbour.shelter" => HarbourAction.Shelter,
            "harbour.depart" => HarbourAction.Depart,
            _ => throw new InvalidOperationException($"unsupported action {action}"),
        };
        var (enabled, next) = state.Apply(mapped);
        state =
            fault == "oil-divergence" && mapped == HarbourAction.LightBeacon
                ? next with
                {
                    Oil = true,
                }
                : next;
        return fault == "wrong-perform"
            ? Variant.From("true")
            : Variant.From(
                fault == "rejected" || (fault == "reject-prefix" && resets > 5) ? false : enabled
            );
    }

    public override void _Ready()
    {
        try
        {
            asset = File.ReadAllBytes(
                ProjectSettings.GlobalizePath("res://native/harbour.recitec")
            );
            var config = new ConfigFile();
            Require(
                config.Load("res://native/harbour.gdextension") == Error.Ok,
                "cannot load extension configuration"
            );
            Require(
                config.Save("user://native.canonical.gdextension") == Error.Ok,
                "cannot serialize extension configuration"
            );
            Require(
                File.ReadAllText(ProjectSettings.GlobalizePath("res://native/harbour.gdextension"))
                    == File.ReadAllText(
                        ProjectSettings.GlobalizePath("user://native.canonical.gdextension")
                    ),
                "extension configuration needs native Godot serialization"
            );
            for (var index = 0; index < 8; index++)
            {
                var extension = (Node)ClassDB.Instantiate("HarbourNative").AsGodotObject();
                try
                {
                    Check(extension);
                }
                finally
                {
                    extension.Free();
                }
            }
            Require(controlledProbes == 96, "controlled asset probes were not executed");
            Require(faultProbes == 96, "native host fault probe count changed");
            GD.Print($"Native boundary fault probes: {faultProbes}");
            GD.Print(
                "Native harbour: 30 cases; both endings; full Recite trace replay.\n12 controlled assets; 8 lifetimes passed."
            );
            GetTree().Quit();
        }
        catch (Exception error)
        {
            GD.PrintErr(error);
            GetTree().Quit(3);
        }
    }

    private void Check(Node extension)
    {
        fault = "";
        var audit = extension.Call("audit", this).AsGodotDictionary();
        Require(audit["ok"].AsBool(), audit.ToString());
        Require(
            audit["cases"].AsInt64() == 30
                && audit["states"].AsInt64() == 6
                && audit["host_actions"].AsInt64() == 85,
            "native fixed-input audit cost changed"
        );
        foreach (
            var (query, depth, outcome) in new[]
            {
                ("sheltered", 3, "reached"),
                ("departed", 3, "reached"),
                ("storm-passed", 3, "unreachable"),
                ("sheltered", 0, "inconclusive"),
            }
        )
        {
            Reset();
            var report = extension.Call("replay", this, query, depth).AsGodotDictionary();
            Require(
                report["ok"].AsBool() && report["outcome"].AsString() == outcome,
                report.ToString()
            );
        }
        foreach (
            var (query, depth) in new[] { ("unknown", 3), ("sheltered", -1), ("sheltered", 4) }
        )
            Require(
                !extension.Call("replay", this, query, depth).AsGodotDictionary()["ok"].AsBool(),
                "invalid request accepted"
            );
        foreach (
            var mode in new[]
            {
                "wrong-capture",
                "wrong-perform",
                "reject-prefix",
                "missing",
                "extra",
                "unknown-key",
                "wrong-type",
                "wrong-symbol-type",
                "unknown-ending",
                "stale",
                "oil-divergence",
                "rejected",
            }
        )
        {
            fault = mode;
            resets = 0;
            faultProbes++;
            var failure = extension.Call("audit", this).AsGodotDictionary();
            Require(!failure["ok"].AsBool(), $"fault accepted: {mode}");
            Require(failure["error"].AsString().Length > 0, "missing native failure evidence");
            if (mode is "oil-divergence" or "rejected" or "reject-prefix")
            {
                Require(failure["kind"].AsString() == "conformance_failure", failure.ToString());
                Require(
                    failure["expected"].AsGodotDictionary().Count == 6
                        && failure["actual"].AsGodotDictionary().Count == 6,
                    "lost full divergence snapshots"
                );
                Require(
                    failure["source"].AsGodotDictionary()["line"].AsInt64() > 0,
                    "lost transition source"
                );
                Require(
                    failure["accepted_host_actions"].AsInt64()
                        >= failure["verified_prefix_actions"].AsInt64(),
                    "invalid host progress"
                );
            }
            if (mode == "oil-divergence")
            {
                Require(
                    failure["action"].AsString() == "keeper.light_beacon",
                    "lost divergent action identity"
                );
                Require(
                    !failure["expected"].AsGodotDictionary()["harbour.oil_available"].AsBool()
                        && failure["actual"].AsGodotDictionary()["harbour.oil_available"].AsBool(),
                    "lost divergent oil evidence"
                );
                Require(
                    failure["verified_prefix_actions"].AsInt64() == 1
                        && failure["accepted_host_actions"].AsInt64() == 2,
                    "incorrect retained host progress"
                );
                Require(
                    Capture()
                        .AsGodotDictionary()
                        .RecursiveEqual(failure["actual"].AsGodotDictionary()),
                    "native failure rolled back accepted host effects"
                );
            }
            if (mode is "wrong-type" or "missing")
                Require(
                    failure["kind"].AsString() == "unsupported_mapping"
                        && failure["source"].AsGodotDictionary()["line"].AsInt64() > 0,
                    "lost typed mapping source"
                );
        }
        Reset();
        fault = "rejected";
        Require(
            !extension.Call("replay", this, "sheltered", 3).AsGodotDictionary()["ok"].AsBool(),
            "rejected witness accepted"
        );
        fault = "";
        Reset();
        Perform("keeper.share_warning");
        var stale = Capture().AsGodotDictionary();
        var staleResult = extension.Call("replay", this, "sheltered", 3).AsGodotDictionary();
        Require(
            !staleResult["ok"].AsBool() && staleResult["kind"].AsString() == "rejected_input",
            "stale live host accepted"
        );
        Require(
            Capture().AsGodotDictionary().RecursiveEqual(stale),
            "stale replay mutated live host"
        );
        foreach (
            var (route, ending, actions, boundaries, line) in new[]
            {
                ("beacon", "sheltered", 3, 7, "10000000000000000005"),
                ("ferry", "departed", 3, 7, "10000000000000000006"),
                ("stale", "open", 2, 6, "10000000000000000004"),
            }
        )
        {
            Reset();
            var dialogue = extension.Call("dialogue", this, route, asset).AsGodotDictionary();
            Require(dialogue["ok"].AsBool(), dialogue.ToString());
            Require(
                dialogue["ending"].AsString() == ending
                    && dialogue["host_actions"].AsInt64() == actions
                    && dialogue["attempted_host_actions"].AsInt64() == 3
                    && dialogue["queries"].AsInt64() == 4
                    && dialogue["boundaries"].AsInt64() == boundaries,
                dialogue.ToString()
            );
            Require(
                dialogue["failed_ack"].AsBool() == (route == "stale"),
                "incorrect blocking acknowledgement"
            );
            Require(
                dialogue["selected_lines"].AsString() == "10000000000000000001," + line,
                "wrong selected dialogue lines"
            );
            var trace = dialogue["trace"].AsGodotArray();
            Require(trace.Count == (route == "stale" ? 10 : 11), "trace boundary cost changed");
            var observedQueries = 0;
            var pendingErrors = 0;
            var acknowledgements = 0;
            foreach (var item in trace)
            {
                var boundary = item.AsGodotDictionary();
                Require(
                    boundary["host"].AsGodotDictionary().Count == 6,
                    "trace lost full host state"
                );
                Require(
                    boundary["session_messagepack"].AsByteArray().Length > 0,
                    "trace lost official full session state"
                );
                var origin = boundary["origin"].AsGodotDictionary();
                Require(
                    origin["file"].AsString().Length > 0 && origin["line"].AsInt64() > 0,
                    "trace lost provenance"
                );
                foreach (var observation in boundary["queries"].AsGodotArray())
                {
                    observedQueries++;
                    var query = observation.AsGodotDictionary();
                    Require(
                        query.ContainsKey("value") && query["arguments"].AsGodotArray().Count == 1,
                        "trace lost typed query observation"
                    );
                }
                var outcome = boundary["outcome"].AsGodotDictionary();
                if (outcome["kind"].AsString() == "runtime_failed")
                {
                    pendingErrors++;
                    Require(
                        outcome["code"].AsString() == "effect_pending"
                            && outcome["effect"].AsString()
                                == boundary["pending"].AsGodotDictionary()["id"].AsString(),
                        "blocking request did not remain pending"
                    );
                }
                if (outcome["kind"].AsString() == "acknowledged")
                {
                    acknowledgements++;
                    Require(
                        outcome["succeeded"].AsBool() == (route != "stale"),
                        "trace lost actual acknowledgement"
                    );
                }
            }
            Require(
                observedQueries == 4 && pendingErrors == 1 && acknowledgements == 1,
                "incomplete runtime trace"
            );
            Reset();
            var replayed = extension.Call("dialogue", this, route, asset).AsGodotDictionary();
            Require(
                replayed["ok"].AsBool() && trace.RecursiveEqual(replayed["trace"].AsGodotArray()),
                "fresh Godot session trace diverged"
            );
            if (route == "stale")
                Require(
                    dialogue["dialogue_source"].AsString().Length > 0
                        && dialogue["host_source"].AsString().Length > 0,
                    "lost failed trace sources"
                );
        }
        Reset();
        foreach (
            var name in new[]
            {
                "wrong-effect-argument",
                "wrong-effect-mode",
                "ranged-source",
                "missing-choice",
                "no-blocking",
                "wrong-delivery",
                "unexpected-blocking",
                "traversal-limit",
                "no-guards",
                "no-prompt-line",
                "absolute-source",
                "wildcard",
            }
        )
        {
            Reset();
            controlledProbes++;
            var controlled = File.ReadAllBytes(
                ProjectSettings.GlobalizePath($"res://native/cases/{name}.recitec")
            );
            var report = extension.Call("dialogue", this, "beacon", controlled).AsGodotDictionary();
            var succeeds =
                name
                is "no-guards"
                    or "no-prompt-line"
                    or "absolute-source"
                    or "ranged-source"
                    or "wildcard";
            Require(report["ok"].AsBool() == succeeds, $"controlled asset {name}: {report}");
            if (!succeeds)
                Require(
                    report["kind"].AsString()
                        == (
                            name.StartsWith("wrong-effect-", StringComparison.Ordinal)
                                ? "unsupported_mapping"
                                : "dialogue_failure"
                        ),
                    report.ToString()
                );
            if (succeeds)
            {
                var capturedTrace = report["trace"].AsGodotArray();
                Reset();
                var repeated = extension
                    .Call("dialogue", this, "beacon", controlled)
                    .AsGodotDictionary();
                Require(
                    repeated["ok"].AsBool()
                        && capturedTrace.RecursiveEqual(repeated["trace"].AsGodotArray()),
                    $"controlled fresh replay {name} diverged"
                );
            }
        }
        Reset();
        foreach (var mode in new[] { "oil-divergence", "rejected" })
        {
            Reset();
            fault = mode;
            var failure = extension.Call("dialogue", this, "beacon", asset).AsGodotDictionary();
            Require(
                !failure["ok"].AsBool() && failure["kind"].AsString() == "conformance_failure",
                failure.ToString()
            );
            Require(
                failure["actual"].AsGodotDictionary().RecursiveEqual(Capture().AsGodotDictionary()),
                "dialogue failure rolled back engine effects"
            );
        }
        fault = "";
        foreach (
            var invalid in new[]
            {
                Array.Empty<byte>(),
                asset.Take(4).ToArray(),
                new byte[] { 1, 2, 3 },
            }
        )
        {
            var before = Capture().AsGodotDictionary();
            var rejectedAsset = extension
                .Call("dialogue", this, "beacon", invalid)
                .AsGodotDictionary();
            Require(
                !rejectedAsset["ok"].AsBool()
                    && rejectedAsset["kind"].AsString() == "analysis_failure",
                "malformed asset accepted"
            );
            Require(
                Capture().AsGodotDictionary().RecursiveEqual(before),
                "malformed asset mutated host"
            );
        }
        foreach (var mode in new[] { "query-wrong-type", "query-drift", "request-drift" })
        {
            Reset();
            captures = 0;
            fault = mode;
            var queryFailure = extension
                .Call("dialogue", this, "beacon", asset)
                .AsGodotDictionary();
            Require(!queryFailure["ok"].AsBool(), "query-only host failure accepted");
            Require(
                queryFailure["kind"].AsString()
                    == (mode == "query-wrong-type" ? "unsupported_mapping" : "conformance_failure"),
                queryFailure.ToString()
            );
            Require(
                queryFailure["source"].AsGodotDictionary()["line"].AsInt64() > 0,
                "lost query-only source"
            );
        }
        fault = "";
        Reset();
        foreach (var replacement in new[] { "not_oil", "harbour_ending" })
        {
            var changedCondition = RenameCompiledString(asset, "has_oil", replacement);
            var invalidCondition = extension
                .Call("dialogue", this, "beacon", changedCondition)
                .AsGodotDictionary();
            Require(
                !invalidCondition["ok"].AsBool()
                    && invalidCondition["kind"].AsString() == "unsupported_mapping",
                invalidCondition.ToString()
            );
            Require(
                invalidCondition["binding"].AsString() == replacement
                    && invalidCondition["source"].AsGodotDictionary()["line"].AsInt64() > 0,
                "lost invalid condition signature/source"
            );
            Require(!state.Warning, "unsupported condition applied an earlier effect");
        }
        var changedEffect = RenameCompiledString(asset, "share_warning", "share_mystery");
        var unmappedBefore = Capture().AsGodotDictionary();
        var unmapped = extension
            .Call("dialogue", this, "beacon", changedEffect)
            .AsGodotDictionary();
        Require(
            !unmapped["ok"].AsBool() && unmapped["kind"].AsString() == "unsupported_mapping",
            unmapped.ToString()
        );
        Require(
            unmapped["binding"].AsString() == "share_mystery"
                && unmapped["source"].AsGodotDictionary()["line"].AsInt64() > 0,
            "lost effect signature and dialogue source"
        );
        Require(
            Capture().AsGodotDictionary().RecursiveEqual(unmappedBefore),
            "unmapped effect mutated host"
        );
        Perform("keeper.share_warning");
        var dialogueBefore = Capture().AsGodotDictionary();
        var staleDialogue = extension.Call("dialogue", this, "beacon", asset).AsGodotDictionary();
        Require(
            !staleDialogue["ok"].AsBool() && staleDialogue["kind"].AsString() == "rejected_input",
            "dialogue silently reset live state"
        );
        Require(
            Capture().AsGodotDictionary().RecursiveEqual(dialogueBefore),
            "stale dialogue mutated host"
        );
        Require(
            !extension.Call("dialogue", this, "unknown", asset).AsGodotDictionary()["ok"].AsBool(),
            "unknown dialogue route accepted"
        );
        Require(
            Capture().AsGodotDictionary().RecursiveEqual(dialogueBefore),
            "unknown route mutated host"
        );
        var missing = new Node();
        try
        {
            Require(
                !extension.Call("audit", missing).AsGodotDictionary()["ok"].AsBool(),
                "missing callbacks accepted"
            );
        }
        finally
        {
            missing.Free();
        }
    }

    // Mutate one fixed MessagePack string produced by the official compiler;
    // the actual native decoder remains the authority for structural validity.
    private static byte[] RenameCompiledString(byte[] input, string previous, string replacement)
    {
        var oldBytes = Encoding.UTF8.GetBytes(previous);
        var newBytes = Encoding.UTF8.GetBytes(replacement);
        var index = input.AsSpan().IndexOf(oldBytes);
        Require(
            index > 0
                && oldBytes.Length < 32
                && newBytes.Length < 32
                && input[index - 1] == 0xa0 + oldBytes.Length,
            "fixture name is not a fixed MessagePack string"
        );
        return input[..(index - 1)]
            .Concat(new[] { checked((byte)(0xa0 + newBytes.Length)) })
            .Concat(newBytes)
            .Concat(input[(index + oldBytes.Length)..])
            .ToArray();
    }

    private static void Require(bool condition, string message)
    {
        if (!condition)
            throw new InvalidOperationException(message);
    }
}
