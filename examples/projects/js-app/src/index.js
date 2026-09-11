/**
 * Discord Hello World Bot (Node.js / JavaScript)
 * Responds with "Hello, World!" to commands and greetings.
 */

const { Client, GatewayIntentBits } = require("discord.js");

const token = process.env.DISCORD_TOKEN || "";

if (!token || token.startsWith("YOUR_")) {
  console.error("ERROR: DISCORD_TOKEN is missing or contains placeholder value.");
  process.exit(1);
}

const client = new Client({
  intents: [
    GatewayIntentBits.Guilds,
    GatewayIntentBits.GuildMessages,
    GatewayIntentBits.MessageContent,
  ],
});

client.once("ready", () => {
  console.log(`[js-discord-bot] Ready and logged in as ${client.user.tag}`);
});

client.on("messageCreate", async (message) => {
  if (message.author.bot) return;

  const content = message.content.trim().toLowerCase();

  if (content === "!hello" || content === "!ping" || content === "hello") {
    await message.reply("Hello, World! Box CLI Node.js engine is running smoothly.");
    console.log(`[js-discord-bot] Replied to ${message.author.tag}`);
  }
});

client.login(token);
