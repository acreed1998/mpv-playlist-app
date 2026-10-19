import {
  type MouseEvent,
  type ReactNode,
  type SyntheticEvent,
  useState,
} from "react";
import {
  Accordion,
  AccordionDetails,
  type AccordionSlotsAndSlotProps,
  AccordionSummary,
  type AccordionSummarySlotsAndSlotProps,
} from "@mui/material";
import clsx from "clsx";
import ArrowDropDown from "@mui/icons-material/ArrowDropDown";

type ExpandCollapseProps = {
  accordionSlots?: AccordionSlotsAndSlotProps;
  accordionSummarySlots?: AccordionSummarySlotsAndSlotProps;
  classNames?: {
    accordion?: string;
    accordionSummary?: string;
    accordionDetails?: string;
  };
  children: ReactNode;
  ids: {
    accordion: string;
    accordionSummary: string;
    accordionDetails: string;
  };
  onChange?: (event: SyntheticEvent, expanded: boolean) => void;
  headerContent?: ReactNode;
};

export const ExpandCollapse = ({
  onChange,
  accordionSlots,
  accordionSummarySlots,
  classNames,
  ids,
  children,
  headerContent,
}: ExpandCollapseProps) => {
  const [expanded, setExpaned] = useState(false);

  const handleClick = (_event: MouseEvent) => {
    setExpaned(!expanded);
  };

  return (
    <Accordion
      className={clsx("expand-collapse", classNames?.accordion)}
      expanded={expanded}
      id={ids.accordion}
      onChange={onChange}
      slotProps={{
        ...accordionSlots?.slotProps,
        heading: { role: "heading", ...accordionSlots?.slotProps?.heading },
      }}
      slots={accordionSlots?.slots}
    >
      <AccordionSummary
        id={ids.accordionSummary}
        aria-controls={ids.accordionDetails}
        className={clsx(
          "expand-collapse-summary",
          classNames?.accordionSummary,
        )}
        expandIcon={<ArrowDropDown />}
        focusVisibleClassName={"expand-collapse-summary-focused"}
        onClick={handleClick}
        slotProps={accordionSummarySlots?.slotProps}
        slots={accordionSummarySlots?.slots}
      >
        {headerContent}
      </AccordionSummary>
      <AccordionDetails
        aria-labelledby={ids.accordionDetails}
        className={clsx(
          "expand-collapse-details",
          classNames?.accordionDetails,
        )}
        id={ids.accordionDetails}
      >
        {expanded ? children : null}
      </AccordionDetails>
    </Accordion>
  );
};
