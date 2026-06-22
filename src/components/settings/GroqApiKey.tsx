import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../hooks/useSettings";
import { Input } from "../ui/Input";
import { SettingContainer } from "../ui/SettingContainer";

interface GroqApiKeyProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const GroqApiKey: React.FC<GroqApiKeyProps> = React.memo(
  ({ descriptionMode = "inline", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const storedKey = getSetting("groq_api_key") || "";
    const [value, setValue] = useState(storedKey);

    useEffect(() => {
      setValue(storedKey);
    }, [storedKey]);

    const handleBlur = () => {
      const trimmed = value.trim();
      if (trimmed !== storedKey) {
        updateSetting("groq_api_key", trimmed);
      }
    };

    return (
      <SettingContainer
        title={t("settings.models.groqApiKey.title")}
        description={t("settings.models.groqApiKey.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      >
        <Input
          type="password"
          className="max-w-64"
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onBlur={handleBlur}
          placeholder={t("settings.models.groqApiKey.placeholder")}
          variant="compact"
          disabled={isUpdating("groq_api_key")}
          autoComplete="off"
        />
      </SettingContainer>
    );
  },
);
