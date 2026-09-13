#!/usr/bin/python3
"""Deterministic visual/input target; never logs typed text or credentials."""
import tkinter as tk

root = tk.Tk()
root.title("DonkeyWork Desktop — virtual display proof")
root.attributes("-fullscreen", True)
root.configure(background="#152238")

canvas = tk.Canvas(root, background="#152238", highlightthickness=0)
canvas.pack(fill="both", expand=True)
count = tk.IntVar(value=0)
status = tk.StringVar(value="Button activations: 0")


def activate():
    count.set(count.get() + 1)
    status.set(f"Button activations: {count.get()}")


button = tk.Button(root, text="Click to prove input", command=activate,
                   font=("DejaVu Sans", 18), takefocus=True)
button.place(x=64, y=112, width=320, height=64)
tk.Label(root, textvariable=status, foreground="white", background="#152238",
         font=("DejaVu Sans Mono", 18)).place(x=64, y=190)
tk.Label(root, text="Text entry (kept in this window only):", foreground="white",
         background="#152238", font=("DejaVu Sans", 14)).place(x=64, y=240)
entry = tk.Entry(root, font=("DejaVu Sans Mono", 18))
entry.place(x=64, y=274, width=600, height=48)


def paint(event):
    width, height = event.width, event.height
    canvas.delete("all")
    for x in range(0, width, 128):
        canvas.create_line(x, 350, x, height, fill="#293a54")
    for y in range(350, height, 128):
        canvas.create_line(0, y, width, y, fill="#293a54")
    canvas.create_text(64, 48, anchor="w", fill="white",
                       font=("DejaVu Sans Mono", 22),
                       text=f"DonkeyWork virtual desktop | {width} × {height}")
    canvas.create_text(64, 364, anchor="nw", fill="#8aff80",
                       font=("DejaVu Sans Mono", 12),
                       text="Native-pixel text: 0123456789 AaBbCc / {} [] <> != → ✓")
    for x, y, color in [(0, 0, "#ff3030"), (width - 32, 0, "#30ff30"),
                         (0, height - 32, "#3030ff"),
                         (width - 32, height - 32, "#ffff30")]:
        canvas.create_rectangle(x, y, x + 31, y + 31, fill=color, outline=color)


canvas.bind("<Configure>", paint)
root.mainloop()
