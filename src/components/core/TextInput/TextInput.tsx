import {
  FormControl,
  FormControlProps,
  FormHelperText,
  FormHelperTextProps,
  Input,
  InputLabel,
  InputLabelProps,
  InputProps,
} from "@mui/material";
import { ChangeEvent, ReactNode } from "react";

type TextInputProps = {
  classNames?: {
    helperText?: string;
    input?: string;
    label?: string;
  };
  formControlProps?: Partial<FormControlProps>;
  formHelperTextProps?: Partial<FormHelperTextProps>;
  helperText?: ReactNode;
  ids: {
    helperText?: string;
    input: string;
    label?: string;
  };
  inputLabelProps?: Partial<InputLabelProps>;
  inputProps?: Partial<InputProps>;
  label?: ReactNode;
  value?: string;
};

export const TextInput = ({
  classNames,
  formControlProps,
  formHelperTextProps,
  helperText,
  ids,
  inputLabelProps,
  inputProps,
  label,
  value,
}: TextInputProps) => {
  const onChange = (
    event: ChangeEvent<HTMLTextAreaElement | HTMLInputElement>,
  ) => {
    inputProps?.onChange?.(event);
  };

  return (
    <FormControl {...formControlProps}>
      {label ? (
        <InputLabel
          {...inputLabelProps}
          className={classNames?.label}
          id={ids.label}
          htmlFor={ids.input}
          shrink={Boolean(value)}
        >
          {label}
        </InputLabel>
      ) : null}

      <Input
        {...inputProps}
        className={classNames?.input}
        id={ids.input}
        aria-describedby={ids.helperText}
        onChange={onChange}
        value={value}
      />

      {helperText ? (
        <FormHelperText
          {...formHelperTextProps}
          className={classNames?.helperText}
          id={ids.helperText}
        >
          {helperText}
        </FormHelperText>
      ) : null}
    </FormControl>
  );
};
