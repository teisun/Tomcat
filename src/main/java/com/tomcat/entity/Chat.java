package com.tomcat.entity;

import com.unfbx.chatgpt.entity.chat.Message;
import lombok.Data;

import java.util.List;
/**
 * 描述：
 *
 */
@Data
public class Chat {

    private String uid;

    private List<Message> message;
}
