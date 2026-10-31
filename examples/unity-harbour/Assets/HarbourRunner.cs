using Harbour;
using UnityEngine;

// Each component owns its live managed harbour; reports never hydrate that state.
public sealed class HarbourRunner : MonoBehaviour
{
    private Consumer consumer;

    private void Awake()
    {
        consumer = new Consumer(
            message => Debug.Log(message),
            message => Debug.LogWarning(message)
        );
    }

    public int Replay(string[] paths) => consumer.Run(paths);
}
