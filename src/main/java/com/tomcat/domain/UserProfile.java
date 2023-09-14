package com.tomcat.domain;

import lombok.Data;

import javax.persistence.*;

@Entity
@Data
@Table(name = "user_profile")
public class UserProfile {

  @Id
  private String id;

  @OneToOne
  @JoinColumn(name="userId", unique = true)
  private User user;

  @Column(columnDefinition = "varchar(255) CHARACTER SET utf8mb4")
  private String nativeLanguage;

  @Column(columnDefinition = "varchar(255) CHARACTER SET utf8mb4")
  private String depth;

  @Column(columnDefinition = "varchar(255) CHARACTER SET utf8mb4")
  private String style;

  @Column(columnDefinition = "varchar(255) CHARACTER SET utf8mb4")
  private String targetLanguage;

  // constructor, getter and setter


  public void setUser(User user) {
    this.user = user;
    this.id = user.getId();
  }
}