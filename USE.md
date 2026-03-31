# How to Use Vertebric

Vertebric is an AI coding assistant that runs in your terminal. You give it a task (like "fix this bug" or "create a simple timer app"), and it acts like a developer — it reads your files, writes code, runs terminal commands to test it, and fixes any errors along the way until the task is complete.

## 1. Get an API Key

Vertebric needs an AI model to power its brain. You can use your favorite provider:

- **Anthropic (Claude):** Get an API key from Anthropic and set `ANTHROPIC_API_KEY`.
- **OpenAI (ChatGPT):** Get an API key from OpenAI and set `OPENAI_API_KEY`.
- **Google (Gemini):** Get an API key from Google AI Studio and set `GEMINI_API_KEY`.
- **Local/Custom (Ollama, LMStudio, etc):** Set `CUSTOM_API_BASE` pointing to your local OpenAI-compatible endpoint.

*Tip: You can set the key in your terminal like this before running a command:*
```bash
export GEMINI_API_KEY="your-api-key-here"
```

## 2. Basic Usage

Run Vertebric from your project's folder. Use the `-p` (or `--prompt`) flag to tell it what you want it to do.

By default, Vertebric uses Anthropic's Claude.

```bash
# Example: Ask it to organize your rust files
vertebric -p "can you find all the unused variables in my code and fix them?"
```

## 3. Using Different AI Models

You can easily switch the "brain" by using the `--provider` and `--model` flags:

```bash
# Using OpenAI's GPT-4o
vertebric -p "build a simple express.js server" --provider openai --model gpt-4o

# Using Google's Gemini 2.5 Flash
vertebric -p "write a python script to rename all images in this folder to numbers" --provider gemini --model gemini-2.5-flash

# Using a local model via Ollama (No API key needed!)
export CUSTOM_API_BASE=http://localhost:11434/v1
vertebric -p "refactor this function to be cleaner" --provider custom --model llama3
```

## 4. Helpful Options

Vertebric has extra flags to help you control how it behaves:

- **`-v` or `--verbose`**: Shows you exactly what the AI is thinking, including the cost of each step and the output of the tools it's using.
- **`--max-turns`**: Sometimes the AI gets stuck in a loop trying to fix a hard bug. This sets a hard limit on how many actions it can take before giving up automatically. Example: `--max-turns 10`.
- **`--max-budget`**: Sets a maximum dollar amount you are willing to spend on this specific task. Once the limit is reached, Vertebric stops. Example: `--max-budget 0.50` (stops when the task costs 50 cents).
- **`--system-prompt`**: Add custom rules for how the AI should behave for this execution. Example: `--system-prompt "Always write code in Python 2"`.

## 5. Example Workflow

Let's say you want to build a small React component:

1. Open your terminal and go to your frontend repository.
2. Run your command:
   ```bash
   vertebric -p "Look at button.tsx, and build a new Toggle component based on that style. Save it to toggle.tsx" -v
   ```
3. Vertebric will read `button.tsx`, understand the style, create `toggle.tsx`, write the code, and print the results to your screen!
