import {
  Fragment,
  MouseEvent,
  useId,
  useRef,
  useState,
  type ReactNode,
} from "react";
import Button, { ButtonProps } from "@mui/material/Button";
import ButtonGroup, { ButtonGroupProps } from "@mui/material/ButtonGroup";
import ArrowDropDownIcon from "@mui/icons-material/ArrowDropDown";
import ClickAwayListener from "@mui/material/ClickAwayListener";
import Grow from "@mui/material/Grow";
import Paper from "@mui/material/Paper";
import Popper from "@mui/material/Popper";
import MenuItem from "@mui/material/MenuItem";
import MenuList from "@mui/material/MenuList";

type SplitButtonsProps = {
  buttonGroupProps?: ButtonGroupProps;
  buttonData: {
    key: string;
    label: ReactNode;
    onClick: (event: MouseEvent) => void;
    otherProps?: ButtonProps;
  }[];
};

export const SplitButtons = ({
  buttonGroupProps,
  buttonData,
}: SplitButtonsProps) => {
  const componentId = useId();
  const [open, setOpen] = useState(false);
  const anchorRef = useRef<HTMLDivElement>(null);
  const [selectedIndex, setSelectedIndex] = useState(0);

  const ids = {
    mainButton: `button-group-main-button-${componentId}`,
    menuList: `button-group-menu-list-${componentId}`,
    openDropdownButton: `button-group-open-dropdown-button-${componentId}`,
  };

  const handleClick = (event: MouseEvent) => {
    buttonData[selectedIndex]?.onClick?.(event);
  };

  const handleMenuItemClick = (_event: MouseEvent, index: number) => {
    setSelectedIndex(index);
    setOpen(false);
  };

  const handleToggle = (_event: MouseEvent) => {
    setOpen((prevOpen) => !prevOpen);
  };

  const handleClose = (event: Event) => {
    if (
      anchorRef.current &&
      anchorRef.current.contains(event.target as HTMLElement)
    ) {
      return;
    }

    setOpen(false);
  };

  return (
    <Fragment>
      <ButtonGroup
        ref={anchorRef}
        {...buttonGroupProps}
        sx={{ button: { textTransform: "none" }, ...buttonGroupProps?.sx }}
      >
        <Button
          id={ids.mainButton}
          onClick={handleClick}
          {...buttonData[selectedIndex]?.otherProps}
        >
          {buttonData[selectedIndex]?.label}
        </Button>
        <Button
          aria-controls={open ? ids.menuList : undefined}
          aria-expanded={open ? "true" : "false"}
          aria-haspopup="menu"
          id={ids.openDropdownButton}
          onClick={handleToggle}
          size="small"
        >
          <ArrowDropDownIcon />
        </Button>
      </ButtonGroup>
      <Popper
        disablePortal
        anchorEl={anchorRef.current}
        open={open}
        role={undefined}
        sx={{ zIndex: 1 }}
        transition
      >
        {({ TransitionProps, placement }) => (
          <Grow
            {...TransitionProps}
            style={{
              transformOrigin:
                placement === "bottom" ? "center top" : "center bottom",
            }}
          >
            <Paper>
              <ClickAwayListener onClickAway={handleClose}>
                <MenuList id={ids.menuList} autoFocusItem>
                  {buttonData.map((data, index) => (
                    <MenuItem
                      key={data.key}
                      onClick={(event) => handleMenuItemClick(event, index)}
                      selected={index === selectedIndex}
                    >
                      {data.label}
                    </MenuItem>
                  ))}
                </MenuList>
              </ClickAwayListener>
            </Paper>
          </Grow>
        )}
      </Popper>
    </Fragment>
  );
};
