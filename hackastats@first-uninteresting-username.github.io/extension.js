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
  const message = Soup.Message.new(
    "GET",
    `${baseUrl}/users/current/statusbar/today?api_key=${apiKey}`,
  );

  const bytes = await session.send_and_read_async(
    message,
    GLib.PRIORITY_DEFAULT,
    cancellable,
  );

  if (message.get_status() !== Soup.Status.OK)
    throw new Error(`HTTP ${message.get_status()}`);

  const text = new TextDecoder().decode(bytes.get_data());
  const json = JSON.parse(text);
  return json.data.grand_total.text;
}

function getPosition(positionInt) {
  if (positionInt === 0) {
    return {
      position: "left",
      index: -1,
    };
  } else if (positionInt === 1) {
    return {
      position: "center",
      index: 0,
    };
  }
  return {
    position: "right",
    index: 0,
  };
}

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
        text: "Loading...",
        x_align: Clutter.ActorAlign.CENTER,
        y_align: Clutter.ActorAlign.CENTER,
      });

      this.add_child(this._label);
      this.refresh();
    }
    async refresh() {
      this._cancellable?.cancel();
      this._cancellable = new Gio.Cancellable();
      const cancellable = this._cancellable;

      let apiKey, baseUrl;

      try {
        const home = GLib.get_home_dir();
        const configFile = new GLib.KeyFile();
        const filePath = `${home}/.wakatime.cfg`;

        let haveFile = true;
        try {
          configFile.load_from_file(filePath, GLib.KeyFileFlags.NONE);
        } catch (e) {
          haveFile = false;
        }

        if (haveFile && hasKey(configFile, "settings", "api_key")) {
          apiKey = configFile.get_string("settings", "api_key");
        } else {
          apiKey = this._settings.get_string("api-key");
        }

        if (haveFile && hasKey(configFile, "settings", "api_url")) {
          baseUrl = configFile.get_string("settings", "api_url");
        } else {
          baseUrl = this._settings.get_string("base-url");
        }

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
          this._label?.set_text("No API key");
        } else {
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

  _reposition() {
    if (!this._settings || !this._session) return;

    if (this._indicator) {
      this._indicator.destroy();
      this._indicator = null;
    }

    const { position, index } = getPosition(this._settings.get_int("position"));
    this._indicator = new Indicator(this._settings, this._session);
    Main.panel.addToStatusArea(this.uuid, this._indicator, index, position);
  }

  enable() {
    this._session = new Soup.Session();

    this._settings = this.getSettings();

    this._settings.connectObject(
      "changed::api-key",
      () => this._indicator?.refresh(),
      "changed::base-url",
      () => this._indicator?.refresh(),
      "changed::refresh-interval",
      () => this._restartTimer(),
      "changed::position",
      () => this._reposition(),
      this,
    );

    this._reposition();

    this._restartTimer();
  }

  disable() {
    if (this._timer) {
      GLib.Source.remove(this._timer);
      this._timer = null;
    }

    this._indicator?.destroy();
    this._indicator = null;
    this._settings?.disconnectObject(this);
    this._settings = null;
    this._session?.abort();
    this._session = null;
  }
}
