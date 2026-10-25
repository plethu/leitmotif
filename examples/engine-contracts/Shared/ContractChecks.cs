namespace Harbour;

// Owns independently executed host state; received snapshots are comparison oracles only.
internal sealed class ContractChecks
{
    private readonly HarbourState initial;
    private HarbourState state;

    public ContractChecks(HarbourState initial)
    {
        this.initial = initial;
        state = initial;
    }

    private bool Apply(HarbourAction action)
    {
        var (enabled, next) = state.Apply(action);
        state = next;
        return enabled;
    }

    private static void Conform(bool condition, string message)
    {
        if (!condition)
            throw new HostDifference(message);
    }

    public int Check(Evidence evidence)
    {
        var identities = new HashSet<(HarbourState, HarbourAction)>();
        foreach (var item in evidence.Cases)
        {
            state = initial;
            foreach (var action in item.Prefix)
                Conform(Apply(action), "disabled audit prefix");
            Conform(state == item.Before, "audit prefix diverged");
            Wire.Require(identities.Add((state, item.Action)), "duplicate audit case");
            var enabled = Apply(item.Action);
            Conform(enabled == item.Enabled, $"{item.Action}: action availability diverged");
            Conform(
                state == item.Expected,
                $"{item.Action}: host action diverged at {item.Source}"
            );
        }
        state = initial;
        switch (evidence.Verdict)
        {
            case Verdict.Unreachable:
                return 1;
            case Verdict.Inconclusive:
                return 2;
            case Verdict.Reached:
                break;
        }
        foreach (var step in evidence.Steps)
        {
            Conform(Apply(step.Action), "disabled replay action");
            Conform(state == step.Expected, "replay successor diverged");
        }
        var satisfied = evidence.Query switch
        {
            "sheltered" => state.Ending == Ending.Sheltered,
            "departed" => state.Ending == Ending.Departed,
            _ => state.Storm,
        };
        Conform(satisfied, "trace does not reach requested target");
        return 0;
    }
}
