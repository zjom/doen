self:
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.programs.doen;
  yaml = pkgs.formats.yaml { };

  # null means "not set": doen applies its own default
  opt =
    type: description:
    lib.mkOption {
      type = lib.types.nullOr type;
      default = null;
      inherit description;
    };

  # doen reads every scalar as a string, so a literal `null` would be rejected
  settings = lib.filterAttrs (_: v: v != null) cfg.settings;

  # git_autocommit shells out to git
  finalPackage =
    if cfg.settings.git_autocommit == true then
      pkgs.symlinkJoin {
        name = "doen-with-git-${cfg.package.version}";
        paths = [ cfg.package ];
        nativeBuildInputs = [ pkgs.makeWrapper ];
        postBuild = ''
          wrapProgram $out/bin/doen --suffix PATH : ${lib.makeBinPath [ pkgs.git ]}
        '';
        inherit (cfg.package) meta;
      }
    else
      cfg.package;
in
{
  options.programs.doen = {
    enable = lib.mkEnableOption "doen, a notebook cli";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.doen;
      defaultText = lib.literalExpression "doen.packages.\${system}.doen";
      description = "The doen package to use.";
    };

    notesDir = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      example = "\${config.home.homeDirectory}/notes";
      description = "The store location, exported as `DOEN_NOTES_DIR`. If null, doen uses `~/doen`.";
    };

    settings = lib.mkOption {
      default = { };
      description = ''
        Written to {file}`$XDG_CONFIG_HOME/doen/config.yaml`.
        A key in `<store>/.doen.yaml` replaces the same key here.
      '';
      type = lib.types.submodule {
        freeformType = yaml.type;
        options = {
          editor = opt lib.types.str "The editor command. If null, doen uses `$VISUAL`, then `$EDITOR`, then `vi`.";
          date_format = opt lib.types.str "The strftime format for absolute dates. doen default: `%y-%m-%d`.";
          week_start = opt (lib.types.enum [
            "mon"
            "tue"
            "wed"
            "thu"
            "fri"
            "sat"
            "sun"
          ]) "The first day of the week. doen default: `mon`.";
          relative_days = opt lib.types.ints.unsigned "Dates within this many days of today show as relative dates. doen default: 7.";
          default_tags = opt (lib.types.listOf lib.types.str) "Tags added to every new item. doen default: none.";
          default_sort = opt (lib.types.enum [
            "due"
            "date"
            "created"
            "closed"
            "title"
            "status"
          ]) "The default sort field of `doen ls`. doen default: not set.";
          archive_closed = opt lib.types.bool "Archive a todo when it is done or dropped. doen default: true.";
          trash_days = opt lib.types.ints.unsigned "Days before doen deletes an item in the trash. doen default: 30.";
          git_autocommit = opt lib.types.bool "Commit the store after each change. If true, git is added to doen's PATH. doen default: false.";
          color = opt (lib.types.enum [
            "auto"
            "always"
            "never"
          ]) "When to color the output. doen default: `auto`.";
          ls_header = opt lib.types.bool "Print the banner above `doen ls`. doen default: true.";
          ls_footer = opt lib.types.bool "Print the cheatsheet below `doen ls`. doen default: true.";
          templates_dir = opt lib.types.str "The global templates directory. A relative path is relative to the store. doen default: `$XDG_CONFIG_HOME/doen/templates`.";
        };
      };
    };

    templates = lib.mkOption {
      type = lib.types.attrsOf (lib.types.either lib.types.lines lib.types.path);
      default = { };
      example = lib.literalExpression ''
        {
          todo = "---\ntags: [inbox]\n---\n";
          meeting = ./templates/meeting.md;
        }
      '';
      description = ''
        Global templates, written to {file}`$XDG_CONFIG_HOME/doen/templates/<name>.md`.
        The names `note` and `todo` are the default templates of their kind.
        doen ignores these templates if `settings.templates_dir` is set.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ finalPackage ];

    home.sessionVariables = lib.mkIf (cfg.notesDir != null) { DOEN_NOTES_DIR = cfg.notesDir; };

    xdg.configFile = {
      "doen/config.yaml" = lib.mkIf (settings != { }) {
        source = yaml.generate "doen-config.yaml" settings;
      };
    }
    // lib.mapAttrs' (
      name: tpl:
      lib.nameValuePair "doen/templates/${name}.md" (
        if builtins.isPath tpl then { source = tpl; } else { text = tpl; }
      )
    ) cfg.templates;
  };
}
