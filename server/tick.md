### One tick (based on kiomet)
1. player joined
    1. Regulator join (maybe fast path)
2. command
    1. Validation
    2. Apply inputs
    3. Generate **events**
3. get alive/alias/score/team
4. player left
    1. Regulator leave
    2. Mark as *dead*
5. tick
    1. Compute scores
    2. Apply **events**
    3. Regulator tick
6. get client/bot update
7. post update
    1. Clear outbound buffers
    2. **Tick boundary**
    3. Maintenance "inputs"
       1. Clean up *dead* players
       2. Shrink the world
    4. Simulate the world
       1. Generate **events**
       2. Mark some players *dead*