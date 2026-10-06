import { useTranslation } from "react-i18next";
import { Button } from "@/shared/ui/button";

export interface Action {
  labelKey: string;
  /** The REPL command, shown as the shortcut hint unless `hint` is set. */
  command: string;
  hint?: string;
  variant?: "default" | "outline" | "secondary" | "ghost";
}

interface ActionBarProps {
  label: string;
  actions: readonly Action[];
  disabled: boolean;
  onRun: (command: string) => void;
}

export function ActionBar({ label, actions, disabled, onRun }: ActionBarProps) {
  const { t } = useTranslation();
  return (
    <div role="group" aria-label={label} className="grid grid-cols-2 gap-1.5">
      {actions.map(({ labelKey, command, hint, variant = "outline" }) => (
        <Button
          key={command}
          size="sm"
          variant={variant}
          disabled={disabled}
          onClick={() => {
            onRun(command);
          }}
          className="justify-between"
        >
          {t(labelKey)}
          <kbd aria-hidden="true" className="font-mono text-[0.7rem] opacity-70">
            {hint ?? command}
          </kbd>
        </Button>
      ))}
    </div>
  );
}
