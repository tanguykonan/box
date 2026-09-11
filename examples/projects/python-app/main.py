"""
Discord Message Archiver Bot (Python + Persistent Volume)

Logs incoming Discord user messages to a persistent JSON store on a mounted volume.
"""

import json
import os
import sys
from datetime import datetime, timezone
import discord
from discord.ext import commands

TOKEN = os.environ.get("DISCORD_TOKEN", "")
DATA_DIR = os.environ.get("DATA_DIR", "/data")
STORAGE_FILE = os.path.join(DATA_DIR, "messages.json")

os.makedirs(DATA_DIR, exist_ok=True)

if not os.path.exists(STORAGE_FILE):
    with open(STORAGE_FILE, "w", encoding="utf-8") as f:
        json.dump([], f, indent=2)

intents = discord.Intents.default()
intents.message_content = True

bot = commands.Bot(command_prefix="!", intents=intents)


def save_message_record(author: str, author_id: int, channel: str, content: str):
    """Appends a new message record to the persistent JSON storage."""
    record = {
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "author": author,
        "author_id": author_id,
        "channel": channel,
        "content": content,
    }

    try:
        with open(STORAGE_FILE, "r", encoding="utf-8") as f:
            data = json.load(f)
    except (json.JSONDecodeError, FileNotFoundError):
        data = []

    data.append(record)

    with open(STORAGE_FILE, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=2)


@bot.event
async def on_ready():
    print(f"[python-discord-logger] Bot connected as {bot.user} (ID: {bot.user.id})")
    print(f"[python-discord-logger] Logging messages to: {STORAGE_FILE}")


@bot.event
async def on_message(message: discord.Message):
    if message.author == bot.user:
        return

    save_message_record(
        author=str(message.author),
        author_id=message.author.id,
        channel=str(message.channel),
        content=message.content,
    )
    print(f"Recorded message from {message.author}: {message.content}")

    await bot.process_commands(message)


@bot.command(name="stats")
async def stats_command(ctx):
    """Returns the total number of logged messages from the persistent volume."""
    try:
        with open(STORAGE_FILE, "r", encoding="utf-8") as f:
            data = json.load(f)
        count = len(data)
    except Exception:
        count = 0
    await ctx.send(f"Total messages logged in persistent volume: {count}")


def main():
    if not TOKEN or TOKEN.startswith("YOUR_"):
        print("ERROR: DISCORD_TOKEN is not set or contains placeholder value.", file=sys.stderr)
        sys.exit(1)

    print(f"Starting Discord Logger Bot with volume target: {DATA_DIR}")
    bot.run(TOKEN)


if __name__ == "__main__":
    main()
