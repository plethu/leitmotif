using System.Text.Json;

namespace Harbour;

internal sealed class HostDifference(string message) : Exception(message);

internal sealed class Rejected(string message) : Exception(message);

internal static class Wire
{
    public const string Schema = "leitmotif.harbour.engine.experimental.v1";
    public const string Binding = "harbour.six-facts-five-actions.v1";
    public const string Model =
        "sha256:f999232901e39df083d73a0f8013f02c20c053c0b0efbafa1f212b9e70ee0b8f";
    public static readonly string[] Facts =
    [
        "harbour.ending",
        "harbour.oil_available",
        "harbour.storm_passed",
        "keeper.warning_shared",
        "keeper.beacon_lit",
        "ferryman.ready",
    ];

    public static void Require(bool condition, string message)
    {
        if (!condition)
            throw new Rejected(message);
    }

    public static void Shape(JsonElement value, params string[] keys)
    {
        var actual = value.EnumerateObject().Select(p => p.Name).OrderBy(value => value).ToArray();
        Require(
            actual.SequenceEqual(keys.OrderBy(value => value)),
            "missing, duplicate or unknown fields"
        );
    }

    public static HarbourState State(JsonElement value, bool tagged)
    {
        Shape(value, Facts);
        JsonElement Fact(string key)
        {
            var item = value.GetProperty(key);
            if (!tagged)
                return item;
            Shape(item, "type", "value");
            Require(
                item.GetProperty("type").GetString()
                    == (key == "harbour.ending" ? "symbol" : "bool"),
                "incorrect snapshot value type"
            );
            return item.GetProperty("value");
        }
        var ending = Fact("harbour.ending").GetString() switch
        {
            "open" => Ending.Open,
            "sheltered" => Ending.Sheltered,
            "departed" => Ending.Departed,
            _ => throw new Rejected("unknown ending"),
        };
        return new(
            ending,
            Fact("harbour.oil_available").GetBoolean(),
            Fact("harbour.storm_passed").GetBoolean(),
            Fact("keeper.warning_shared").GetBoolean(),
            Fact("keeper.beacon_lit").GetBoolean(),
            Fact("ferryman.ready").GetBoolean()
        );
    }

    public static HarbourState Snapshot(JsonElement value)
    {
        Shape(value, "values");
        return State(value.GetProperty("values"), true);
    }

    public static HarbourAction Action(string? id) =>
        id switch
        {
            "keeper.share_warning" => HarbourAction.ShareWarning,
            "keeper.light_beacon" => HarbourAction.LightBeacon,
            "ferryman.fuel_ferry" => HarbourAction.FuelFerry,
            "harbour.shelter" => HarbourAction.Shelter,
            "harbour.depart" => HarbourAction.Depart,
            _ => throw new Rejected("unsupported action"),
        };
}
