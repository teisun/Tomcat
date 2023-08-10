package com.tomcat.domain;

import javax.persistence.*;

@Entity
@Table(name = "user_profile")
public class UserProfile {

  @Id
  @GeneratedValue
  private Long id;

  @OneToOne
  @JoinColumn(name="userId")
  private User user;

  private String motherTongue;

  private String languageDepth;

  private String communicationStyle;

  private String targetLanguage;

  // constructor, getter and setter


  public Long getId() {
    return id;
  }

  public void setId(Long id) {
    this.id = id;
  }

  public String getMotherTongue() {
    return motherTongue;
  }

  public void setMotherTongue(String motherTongue) {
    this.motherTongue = motherTongue;
  }

  public String getLanguageDepth() {
    return languageDepth;
  }

  public void setLanguageDepth(String languageDepth) {
    this.languageDepth = languageDepth;
  }

  public String getCommunicationStyle() {
    return communicationStyle;
  }

  public void setCommunicationStyle(String communicationStyle) {
    this.communicationStyle = communicationStyle;
  }

  public String getTargetLanguage() {
    return targetLanguage;
  }

  public void setTargetLanguage(String targetLanguage) {
    this.targetLanguage = targetLanguage;
  }
}