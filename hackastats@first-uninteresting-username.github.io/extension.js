/* extension.js
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 2 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

import Gio from "gi://Gio";
import GObject from "gi://GObject";
import St from "gi://St";
import Clutter from "gi://Clutter";
import Soup from "gi://Soup";
import GLib from "gi://GLib";

import {
  Extension,
  gettext as _,
} from "resource:///org/gnome/shell/extensions/extension.js";
import * as PanelMenu from "resource:///org/gnome/shell/ui/panelMenu.js";

import * as Main from "resource:///org/gnome/shell/ui/main.js";

async function getToday(session, baseUrl, apiKey, cancellable) {
  // Create a request
  const message = Soup.Message.new(
    "GET",
    `${baseUrl}/users/current/statusbar/today?api_key=${apiKey}`,
  );

  const bytes = await session.send_and_read_async(
    message,
    GLib.PRIORITY_DEFAULT,
    cancellable,
  );

  // Error on non ok http response codes
  if (message.get_status() !== Soup.Status.OK)
    throw new Error(`HTTP ${message.get_status()}`);

  // Convert bytes to json
  const text = new TextDecoder().decode(bytes.get_data());
  const json = JSON.parse(text);
  // https://wakatime.com/developers#status_bar or https://hackatime.hackclub.com/api-docs#tag/wakatime-compatibility/GET/api/hackatime/v1/users/{id}/statusbar/today
  return json.data.grand_total.text;
}

// Position on the panel
function getPosition(positionInt) {
  if (positionInt === 0) {
    return {
      // Rightmost on the left side
      position: "left",
      index: -1,
    };
  } else if (positionInt === 1) {
    return {
      // Leftmost on the center
      position: "center",
      index: 0,
    };
  }
  return {
    position: "right",
    index: 0,
  };
}

// Check if the file has some key
function hasKey(keyFile, group, key) {
  try {
    keyFile.get_value(group, key);
    return true;
  } catch (e) {
    return false;
  }
}

const Indicator = GObject.registerClass(
  class Indicator extends PanelMenu.Button {
    _init(settings, session) {
      super._init(0.0, _("Hackastats indicator"));

      this._settings = settings;
      this._session = session;
      this._cancellable = new Gio.Cancellable();
      this._label = new St.Label({
        // Displayed before first fetch
        text: "Loading...",
        // Align to the center of the box
        x_align: Clutter.ActorAlign.CENTER,
        y_align: Clutter.ActorAlign.CENTER,
      });

      this.add_child(this._label);
      this.refresh();
    }
    // Refresh panel
    async refresh() {
      this._cancellable?.cancel();
      this._cancellable = new Gio.Cancellable();
      const cancellable = this._cancellable;

      let apiKey, baseUrl;

      try {
        // Get ~/.wakatime.cfg
        const home = GLib.get_home_dir();
        const configFile = new GLib.KeyFile();
        const filePath = `${home}/.wakatime.cfg`;

        // Load ~/.wakatime.cfg
        let haveFile = true;
        try {
          configFile.load_from_file(filePath, GLib.KeyFileFlags.NONE);
        } catch (e) {
          haveFile = false;
        }

        // Assign api key
        if (haveFile && hasKey(configFile, "settings", "api_key")) {
          apiKey = configFile.get_string("settings", "api_key");
        } else {
          apiKey = this._settings.get_string("api-key");
        }

        // Assign base url
        if (haveFile && hasKey(configFile, "settings", "api_url")) {
          baseUrl = configFile.get_string("settings", "api_url");
        } else {
          baseUrl = this._settings.get_string("base-url");
        }

        // Get today stats
        const text = await getToday(
          this._session,
          baseUrl,
          apiKey,
          cancellable,
        );
        if (cancellable.is_cancelled()) return;
        this._label?.set_text(text);
      } catch (e) {
        if (
          cancellable.is_cancelled() ||
          (e instanceof GLib.Error &&
            e.matches(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
        )
          return;
        console.error("Hackastats", e);
        if (apiKey == "") {
          // Display that message when API key isn't configured
          this._label?.set_text("No API key");
        } else {
          // Display that message when there's no connection to the server or api key/base url is declared in a wrong way
          // Might be unhelpful
          this._label?.set_text("Server unavailable");
        }
      }
    }

    destroy() {
      this._cancellable?.cancel();
      this._cancellable = null;
      this._session = null;
      this._settings = null;
      this._label = null;
      super.destroy();
    }
  },
);

export default class HackastatsExtension extends Extension {
  // Restart (or enable) the timer that refreshes the data
  _restartTimer() {
    if (this._timer) {
      GLib.Source.remove(this._timer);
      this._timer = null;
    }

    const interval = this._settings.get_int("refresh-interval");
    this._timer = GLib.timeout_add_seconds(
      GLib.PRIORITY_DEFAULT,
      interval,
      () => {
        this._indicator?.refresh();
        return GLib.SOURCE_CONTINUE;
      },
    );
  }

  // Add the indicator to the panel
  _reposition() {
    if (!this._settings || !this._session) return;

    // Destroy the indicator if it exists
    if (this._indicator) {
      this._indicator.destroy();
      this._indicator = null;
    }

    // Create the indicator
    const { position, index } = getPosition(this._settings.get_int("position"));
    this._indicator = new Indicator(this._settings, this._session);
    Main.panel.addToStatusArea(this.uuid, this._indicator, index, position);
  }

  enable() {
    this._session = new Soup.Session();

    this._settings = this.getSettings();

    // Refresh things when dconf is changed
    this._handlerIds = [
      this._settings.connect("changed::api-key", () =>
        this._indicator?.refresh(),
      ),
      this._settings.connect("changed::base-url", () =>
        this._indicator?.refresh(),
      ),
      this._settings.connect("changed::refresh-interval", () =>
        this._restartTimer(),
      ),
      this._settings.connect("changed::position", () => this._reposition()),
    ];

    this._reposition();

    this._restartTimer();
  }

  disable() {
    // Delete the timer
    if (this._timer) {
      GLib.Source.remove(this._timer);
      this._timer = null;
    }

    this._indicator?.destroy();
    this._indicator = null;
    // Close the process that refreshes things in reaction to dconf changes
    if (this._settings && this._handlerIds) {
      for (const id of this._handlerIds) this._settings.disconnect(id);
    }
    this._handlerIds = null;
    this._settings = null;
    this._session?.abort();
    this._session = null;
  }
}
