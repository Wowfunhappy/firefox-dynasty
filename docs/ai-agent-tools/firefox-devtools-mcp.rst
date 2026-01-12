=========================================
Firefox DevTools MCP for AI Agent Testing
=========================================

Overview
========

The ``firefox-devtools-mcp`` is a Model Context Protocol (MCP) server that
enables AI assistants (such as Claude Code) to automate and interact with
Firefox through WebDriver BiDi. This tool is particularly useful for:

- Automating browser interactions for testing and debugging
- Attempting to reproduce issues on live websites
- Capturing Firefox debug logs (MOZ_LOG) while controlling the browser
- Dynamically changing Firefox configuration without restarting

The MCP server provides programmatic access to a Firefox build, including
DevTools capabilities such as page navigation, DOM inspection, network
monitoring, console logs, user input simulation, and Firefox logging. It isn't
complete yet, but can already be useful.

This documentation refers to a Mozilla-specific fork with enhanced logging
capabilities: https://github.com/padenot/firefox-devtools-mcp

Key Features
------------

- **WebDriver BiDi Protocol**: Uses the modern WebDriver BiDi protocol via
  Selenium WebDriver
- **Local Firefox Support**: Can connect to custom Firefox builds (e.g., local
  development builds), or typical release/beta/nightly/esr builds, present on
  the machine
- **Real-time Interaction**: Provides live access to browser state and network
  activity
- **Firefox Output Capture**: stdout/stderr capturing, including setting and reading MOZ_LOG

Prerequisites
=============

Required Software
-----------------

- **Node.js**: Version 20.19.0 or higher
- **Firefox**: system installation (all channels supported) or local build
- **Claude Code** (or compatible MCP client): For AI-assisted automation

Installation and Configuration
===============================

Step 1: Clone and Build the MCP Server
---------------------------------------

First, clone the Mozilla fork with enhanced logging capabilities (we'll try to
upstream those changes):

.. code-block:: bash

   git clone https://github.com/padenot/firefox-devtools-mcp.git
   cd firefox-devtools-mcp
   npm install
   npm run build

Step 2: Add the MCP Server to Claude Code
------------------------------------------

Using Claude Code CLI
~~~~~~~~~~~~~~~~~~~~~

The simplest method is to use the Claude Code command-line interface:

.. code-block:: bash

   claude mcp add firefox-devtools node /path/to/firefox-devtools-mcp/dist/index.js

Replace ``/path/to/firefox-devtools-mcp`` with the actual path where you cloned the repository.

This command registers the MCP server with your Claude Code configuration for the current project.

Manual Configuration
~~~~~~~~~~~~~~~~~~~~

Alternatively, you can manually edit your Claude Code configuration file:

**Configuration file locations:**

- **macOS**: ``~/Library/Application Support/Claude/Code/mcp_settings.json``
- **Linux**: ``~/.config/claude/code/mcp_settings.json``
- **Windows**: ``%APPDATA%\Claude\Code\mcp_settings.json``

Add the following to your project's ``mcpServers`` section:

.. code-block:: json

   {
     "projects": {
       "/path/to/firefox": {
         "mcpServers": {
           "firefox-devtools": {
             "type": "stdio",
             "command": "node",
             "args": [
               "/path/to/firefox-devtools-mcp/dist/index.js",
               "--firefox-path",
               "/path/to/firefox/obj-x86_64-pc-linux-gnu/dist/bin/firefox"
             ],
             "env": {}
           }
         }
       }
     }
   }

Replace paths with your actual locations.

Step 3: Configure Local Firefox Build
--------------------------------------

To use your local Firefox development build, add the ``--firefox-path`` argument
pointing to your compiled Firefox binary, which is in the objdir, e.g. for a
standard Linux build:

.. code-block:: bash

   --firefox-path /path/to/mozilla-central/obj-x86_64-pc-linux-gnu/dist/bin/firefox

Step 4: Configuration Options
------------------------------

The MCP server supports several command-line options:

.. list-table::
   :widths: 25 75
   :header-rows: 1

   * - Option
     - Description
   * - ``--firefox-path <path>``
     - Path to Firefox executable (system Firefox used if not specified)
   * - ``--headless``
     - Run Firefox without UI (useful for automated testing)
   * - ``--viewport <size>``
     - Set initial viewport size (e.g., ``1280x720``)
   * - ``--profile-path <path>``
     - Use a specific Firefox profile directory
   * - ``--start-url <url>``
     - Initial URL to load (default: ``about:home``)
   * - ``--accept-insecure-certs``
     - Ignore TLS certificate errors (use with caution)
   * - ``--firefox-arg <arg>``
     - Additional arguments to pass to Firefox
   * - ``--env <KEY=VALUE>``
     - Set environment variables for Firefox (can be used multiple times)
   * - ``--output-file <path>``
     - Path where Firefox output (stdout/stderr) will be captured (optional)

Example with multiple options:

.. code-block:: json

   {
     "command": "node",
     "args": [
       "/path/to/firefox-devtools-mcp/dist/index.js",
       "--firefox-path",
       "/home/developer/firefox/obj-x86_64-pc-linux-gnu/dist/bin/firefox",
       "--headless",
       "--viewport",
       "1920x1080",
       "--env",
       "MOZ_LOG=HTMLMediaElement:5"
     ]
   }

Step 5: Restart Claude Code
----------------------------

After configuration, restart Claude Code to load the new MCP server:

.. code-block:: bash

   # Exit Claude Code, then restart it, continuing the last session
   claude -c

The MCP server will automatically start when Claude Code initializes.

Available Capabilities
======================

The ``firefox-devtools-mcp`` provides tools through the MCP protocol. Tool names
are generally self-describing. Simply ask your AI assistant to perform tasks - it
will use the appropriate tools. This list may not be complete as the server is
under active development.

**Page Management:**
``list_pages``, ``new_page``, ``navigate_page``, ``select_page``, ``close_page``

**DOM Interaction:**
``take_snapshot``, ``resolve_uid_to_selector``, ``click_by_uid``, ``hover_by_uid``,
``fill_by_uid``, ``fill_form_by_uid``, ``drag_by_uid_to_uid``, ``upload_file_by_uid``

**Network & Console:**
``list_network_requests``, ``get_network_request``, ``list_console_messages``,
``clear_console``

**Screenshots:**
``screenshot_page``, ``screenshot_by_uid``

**Dialogs & Navigation:**
``accept_dialog``, ``dismiss_dialog``, ``navigate_history``, ``set_viewport_size``

**Firefox Management:**
``get_firefox_info``, ``get_firefox_output``, ``restart_firefox``

**Script Evaluation:**
``evaluate_script`` (content), ``evaluate_chrome_script`` (privileged)

**Chrome Context (privileged code):**
``list_chrome_contexts``, ``select_chrome_context``, ``evaluate_chrome_script``

Working with MOZ_LOG
====================

Firefox's MOZ_LOG system provides detailed debug output from various modules.
The ``firefox-devtools-mcp`` captures this output automatically when environment
variables are set.

You can enable MOZ_LOG with something like this:

.. code-block:: text

   Developer: "Enable HTMLMediaElement logging at level 5"

   AI uses: restart_firefox(env=["MOZ_LOG=HTMLMediaElement:5"])

Common MOZ_LOG Modules
-----------------------

For a useful list of available MOZ_LOG modules and their descriptions, see
``toolkit/content/aboutLogging.mjs`` in the Firefox repository, or search
searchfox.org with something like this `searchfox query <https://searchfox.org/firefox-main/search?q=LazyLogModule.*%5C%28%22&path=&case=false&regexp=true>`__.

Chrome Context Access
=====================

For debugging privileged Firefox code (chrome/XUL), enable system access:

.. code-block:: text

   restart_firefox(env=["MOZ_REMOTE_ALLOW_SYSTEM_ACCESS=1"])

Then use ``list_chrome_contexts``, ``select_chrome_context``, and
``evaluate_chrome_script`` to access ``gBrowser``, ``Components``, and other
privileged APIs.
