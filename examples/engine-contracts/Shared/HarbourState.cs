namespace Harbour;

internal enum Ending
{
    Open,
    Sheltered,
    Departed,
}

internal enum HarbourAction
{
    ShareWarning,
    LightBeacon,
    FuelFerry,
    Shelter,
    Depart,
}

internal sealed record HarbourState(
    Ending Ending,
    bool Oil,
    bool Storm,
    bool Warning,
    bool Beacon,
    bool Ferry
)
{
    public (bool Enabled, HarbourState State) Apply(HarbourAction action)
    {
        if (Ending != Ending.Open)
            return (false, this);
        return action switch
        {
            HarbourAction.ShareWarning when !Warning => (true, this with { Warning = true }),
            HarbourAction.LightBeacon when Warning && Oil => (
                true,
                this with
                {
                    Beacon = true,
                    Oil = false,
                }
            ),
            HarbourAction.FuelFerry when Warning && Oil => (
                true,
                this with
                {
                    Ferry = true,
                    Oil = false,
                }
            ),
            HarbourAction.Shelter when Beacon => (true, this with { Ending = Ending.Sheltered }),
            HarbourAction.Depart when Ferry => (true, this with { Ending = Ending.Departed }),
            _ => (false, this),
        };
    }
}
