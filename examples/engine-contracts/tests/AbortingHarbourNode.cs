// Actual Godot lifecycle failure injection: AddChild returns after the engine catches this.
public partial class AbortingHarbourNode : HarbourNode
{
    public override void _Ready() =>
        throw new System.InvalidOperationException("intentional early Node abort");
}
