import matplotlib.pyplot as plt

# Writer TSC values
writer_tsc = [
    2376748235825360,
    2376748240457500,
    2376748245022620,
    2376748249579840,
    2376748254105020,
    2376748258725820,
    2376748263314320,
    2376748267876480,
    2376748272403660,
    2376748276957160,
    2376748281879180,
]

# Reader TSC values
reader_tsc = [
    2376748235817640,
    2376748240458340,
    2376748245023720,
    2376748249580600,
    2376748254106440,
    2376748258727040,
    2376748263315080,
    2376748267877260,
    2376748272404400,
    2376748276957880,
    2376748281899400,
]

# Compute deltas (cycles between checkpoints)
writer_deltas = [writer_tsc[i] - writer_tsc[i-1] for i in range(1, len(writer_tsc))]
reader_deltas = [reader_tsc[i] - reader_tsc[i-1] for i in range(1, len(reader_tsc))]

# X axis (checkpoint index)
x = list(range(1, len(writer_tsc)))

# Plot
plt.figure()

plt.plot(x, writer_deltas, marker='o', label='Writer')
plt.plot(x, reader_deltas, marker='o', label='Reader')

plt.xlabel("Checkpoint")
plt.ylabel("Cycles (delta TSC)")
plt.title("TSC Delta per Checkpoint (Reader vs Writer)")
plt.legend()
plt.grid(True)

plt.show()